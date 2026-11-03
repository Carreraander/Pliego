//! Etiquetado automático inicial de un documento sin etiquetar, con heurísticas sencillas:
//! encabezados por tamaño de letra (o negrita), listas por viñeta o numeración, párrafos por proximidad,
//! figuras por imágenes y el resto de gráficos como artefactos. El orden de lectura es el del contenido.
//!
//! Limitaciones conocidas: no detecta tablas, no etiqueta enlaces ni campos de formulario y los
//! textos alternativos de las figuras los pone la persona (`set_alt_text`). Siempre conviene revisar
//! el resultado con `check`.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use mupdf::Buffer;
use mupdf::pdf::{PdfDocument, PdfObject, PdfWriteOptions};
use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

use crate::content::{SegKind, Segment, TextInfo, segment, tokenize};
use crate::edit::{set_language, set_tab_order, set_title};
use crate::io::{name_of, open_pdf, pdf_err, save_atomic, xref_of};

const MAX_CONTENT_BYTES: usize = 64 * 1024 * 1024;

/// Opciones de [`autotag`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutotagOptions {
    /// Idioma principal (`es-ES`...). Si no se indica, el documento queda sin idioma y `check` lo avisará.
    pub language: Option<String>,
    pub title: Option<String>,
}

/// Qué se etiquetó.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutotagSummary {
    pub headings: usize,
    pub paragraphs: usize,
    pub lists: usize,
    pub list_items: usize,
    pub figures: usize,
    pub artifacts: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Para,
    Heading(u8),
    Label,
    ListBody,
    Figure,
    Artifact,
}

struct Item {
    seg: Segment,
    class: Class,
    size: f32,
    y: Option<f32>,
}

struct PageData {
    page: PdfObject,
    bytes: Vec<u8>,
    items: Vec<Item>,
}

fn read_contents(page: &PdfObject) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let Some(c) = page.get_dict("Contents").map_err(pdf_err)? else {
        return Ok(out);
    };
    let streams = if c.is_array().unwrap_or(false) {
        (0..c.len().map_err(pdf_err)?)
            .filter_map(|i| c.get_array(i as i32).ok().flatten())
            .collect()
    } else {
        vec![c]
    };
    for s in streams {
        if s.is_stream().unwrap_or(false) {
            out.extend(s.read_stream().map_err(pdf_err)?);
            out.push(b'\n');
        }
        if out.len() > MAX_CONTENT_BYTES {
            return Err(Error::Unsupported(
                "el contenido de una página es demasiado grande para etiquetarlo".into(),
            ));
        }
    }
    Ok(out)
}

/// ¿Es toda la cadena una etiqueta de lista (viñeta o numeración)?
fn is_label(s: &[u8]) -> bool {
    let t: Vec<u8> = s.iter().copied().filter(|b| !b.is_ascii_whitespace()).collect();
    if t.is_empty() || t.len() > 5 {
        return false;
    }
    if matches!(t.as_slice(), [0x95] | [0xB7] | [b'-'] | [b'*'] | [0x96] | [0x97]) {
        return true;
    }
    let (body, last) = t.split_at(t.len() - 1);
    let closing = matches!(last[0], b'.' | b')');
    let inner = body.strip_prefix(b"(").unwrap_or(body);
    closing
        && !inner.is_empty()
        && (inner.iter().all(u8::is_ascii_digit) || (inner.len() == 1 && inner[0].is_ascii_alphabetic()))
}

/// ¿Empieza por una etiqueta seguida de texto («• algo», «1. algo»)?
fn starts_with_label(s: &[u8]) -> bool {
    match s.iter().position(|b| *b == b' ') {
        Some(p) if p > 0 && p < s.len() - 1 => is_label(&s[..p]),
        _ => false,
    }
}

fn font_is_bold(page_res: &Option<PdfObject>, name: &str) -> bool {
    let base = page_res
        .as_ref()
        .and_then(|r| r.get_dict("Font").ok().flatten())
        .and_then(|f| f.get_dict(name).ok().flatten())
        .and_then(|f| name_of(&f, "BaseFont"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    base.contains("bold") || base.contains("black") || base.contains("heavy")
}

fn form_class(x: &PdfObject) -> Class {
    let data = x.read_stream().unwrap_or_default();
    let has = |needle: &[u8]| data.windows(needle.len()).any(|w| w == needle);
    if has(b"BT") {
        Class::Para
    } else if has(b" Do") || has(b"BI") {
        Class::Figure
    } else {
        Class::Artifact
    }
}

fn round_half(v: f32) -> f32 {
    (v * 2.0).round() / 2.0
}

fn analyze(doc: &PdfDocument, ctx: &JobContext) -> Result<Vec<PageData>> {
    let pages = doc.page_count().map_err(pdf_err)?.max(0) as usize;
    let mut out = Vec::with_capacity(pages);
    for p in 0..pages {
        ctx.step(p as u64, pages as u64 * 2, "Analizando páginas")?;
        let page = doc.find_page(p as i32).map_err(pdf_err)?;
        let bytes = read_contents(&page)?;
        let toks = tokenize(&bytes);
        let segs = segment(&bytes, &toks);
        let res = page.get_dict_inheritable("Resources").map_err(pdf_err)?;
        let xobjects = res.as_ref().and_then(|r| r.get_dict("XObject").ok().flatten());
        let mut items = Vec::with_capacity(segs.len());
        for seg in segs {
            let (class, size, y) = match &seg.kind {
                SegKind::Path => (Class::Artifact, 0.0, None),
                SegKind::InlineImage => (Class::Figure, 0.0, None),
                SegKind::Xobject(name) => {
                    let x = xobjects.as_ref().and_then(|d| d.get_dict(name.as_str()).ok().flatten());
                    let class = match x.as_ref().and_then(|x| name_of(x, "Subtype")).as_deref() {
                        Some("Image") => Class::Figure,
                        Some("Form") => x.as_ref().map(form_class).unwrap_or(Class::Artifact),
                        _ => Class::Artifact,
                    };
                    (class, 0.0, None)
                }
                SegKind::Text(t) => (Class::Para, t.size, t.y),
            };
            items.push(Item { seg, class, size, y });
        }
        out.push(PageData { page, bytes, items });
    }
    Ok(out)
}

fn text_info(it: &Item) -> Option<&TextInfo> {
    if let SegKind::Text(t) = &it.seg.kind {
        Some(t)
    } else {
        None
    }
}

/// Tamaño de letra más frecuente (ponderado por cantidad de texto).
fn body_size(pages: &[PageData]) -> f32 {
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    for it in pages.iter().flat_map(|p| &p.items) {
        if let Some(t) = text_info(it)
            && t.size > 0.0
        {
            *hist.entry((round_half(t.size) * 2.0) as i32).or_default() += t.len.max(1);
        }
    }
    hist.into_iter()
        .max_by_key(|(_, w)| *w)
        .map(|(k, _)| k as f32 / 2.0)
        .unwrap_or(0.0)
}

fn classify(pages: &mut [PageData], bold: &HashMap<(usize, String), bool>) {
    let body = body_size(pages);
    // Tamaños candidatos a encabezado, de mayor a menor.
    let mut sizes: Vec<i32> = Vec::new();
    for it in pages.iter().flat_map(|p| &p.items) {
        if let Some(t) = text_info(it)
            && body > 0.0
            && t.size >= body * 1.12
            && t.len <= 200
        {
            let k = (round_half(t.size) * 2.0) as i32;
            if !sizes.contains(&k) {
                sizes.push(k);
            }
        }
    }
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    let bold_level = (sizes.len() as u8 + 1).min(6);
    for (pi, page) in pages.iter_mut().enumerate() {
        let mut pending_label = false;
        for it in &mut page.items {
            let SegKind::Text(t) = &it.seg.kind else {
                pending_label = false;
                continue;
            };
            let k = (round_half(t.size) * 2.0) as i32;
            let is_bold = t
                .font
                .as_ref()
                .is_some_and(|f| bold.get(&(pi, f.clone())).copied().unwrap_or(false));
            it.class = if pending_label {
                pending_label = false;
                Class::ListBody
            } else if is_label(&t.sample) && t.len <= 6 {
                pending_label = true;
                Class::Label
            } else if starts_with_label(&t.sample) {
                Class::ListBody
            } else if let Some(rank) = sizes
                .iter()
                .position(|s| *s == k)
                .filter(|_| t.len <= 200 && body > 0.0 && t.size >= body * 1.12)
            {
                Class::Heading((rank as u8 + 1).min(6))
            } else if is_bold && t.len <= 100 && body > 0.0 && t.size >= body * 0.95 && t.size <= body * 1.12 {
                Class::Heading(bold_level)
            } else {
                Class::Para
            };
        }
    }
}

/// Un bloque lógico de la estructura.
#[derive(Debug)]
enum Block {
    Heading(u8, Vec<usize>),
    Para(Vec<usize>),
    ListItem { label: Option<usize>, body: Vec<usize> },
    Figure(usize),
}

fn build_blocks(page: &PageData) -> Vec<Block> {
    // `índice de elemento` = posición en page.items; el MCID se asigna al reescribir (en orden).
    let mut blocks: Vec<Block> = Vec::new();
    let mut label: Option<usize> = None;
    let near = |a: &Item, b: &Item| match (a.y, b.y) {
        (Some(ya), Some(yb)) => (ya - yb).abs() <= 2.0 * a.size.max(1.0) && (a.size - b.size).abs() < 0.5,
        _ => false,
    };
    let mut last_idx: Option<usize> = None;
    for (i, it) in page.items.iter().enumerate() {
        if it.class != Class::ListBody
            && let Some(l) = label.take()
        {
            // Etiqueta sin texto detrás: se trata como un párrafo.
            blocks.push(Block::Para(vec![l]));
        }
        match it.class {
            Class::Artifact => {}
            Class::Figure => blocks.push(Block::Figure(i)),
            Class::Label => label = Some(i),
            Class::ListBody => {
                blocks.push(Block::ListItem {
                    label: label.take(),
                    body: vec![i],
                });
                last_idx = Some(i);
                continue;
            }
            Class::Heading(l) => match (blocks.last_mut(), last_idx) {
                (Some(Block::Heading(pl, v)), Some(prev)) if *pl == l && near(&page.items[prev], it) => v.push(i),
                _ => blocks.push(Block::Heading(l, vec![i])),
            },
            Class::Para => match (blocks.last_mut(), last_idx) {
                (Some(Block::Para(v)), Some(prev)) if near(&page.items[prev], it) => v.push(i),
                (Some(Block::ListItem { body, .. }), Some(prev))
                    if near(&page.items[prev], it) && it.y < page.items[prev].y =>
                {
                    body.push(i)
                }
                _ => blocks.push(Block::Para(vec![i])),
            },
        }
        last_idx = Some(i);
    }
    if let Some(l) = label {
        blocks.push(Block::Para(vec![l]));
    }
    blocks
}

fn new_name(s: &str) -> Result<PdfObject> {
    PdfObject::new_name(s).map_err(pdf_err)
}

fn new_int(v: i32) -> Result<PdfObject> {
    PdfObject::new_int(v).map_err(pdf_err)
}

fn make_elem(
    doc: &mut PdfDocument,
    role: &str,
    parent: &PdfObject,
    page: &PdfObject,
    kids: Option<PdfObject>,
) -> Result<PdfObject> {
    let mut d = doc.new_dict().map_err(pdf_err)?;
    d.dict_put("Type", new_name("StructElem")?).map_err(pdf_err)?;
    d.dict_put("S", new_name(role)?).map_err(pdf_err)?;
    d.dict_put("P", parent.try_clone().map_err(pdf_err)?).map_err(pdf_err)?;
    if !page.is_null().unwrap_or(true) {
        d.dict_put("Pg", page.try_clone().map_err(pdf_err)?).map_err(pdf_err)?;
    }
    if let Some(k) = kids {
        d.dict_put("K", k).map_err(pdf_err)?;
    }
    doc.add_object(&d).map_err(pdf_err)
}

fn push_kid(elem: &mut PdfObject, kid: PdfObject) -> Result<()> {
    let mut kids = match elem.get_dict("K").map_err(pdf_err)? {
        Some(k) if k.is_array().unwrap_or(false) => k,
        _ => {
            let a = elem
                .document()
                .ok_or_else(|| Error::Pdf("objeto sin documento".into()))?
                .new_array()
                .map_err(pdf_err)?;
            elem.dict_put("K", a).map_err(pdf_err)?;
            elem.get_dict("K")
                .map_err(pdf_err)?
                .ok_or_else(|| Error::Pdf("no se pudo crear K".into()))?
        }
    };
    kids.array_push(kid).map_err(pdf_err)
}

fn bytes_buffer(data: &[u8]) -> Result<Buffer> {
    let mut b = Buffer::with_capacity(data.len());
    std::io::Write::write_all(&mut b, data)?;
    Ok(b)
}

/// Etiqueta un documento abierto. Falla si ya tiene estructura.
pub fn autotag(doc: &mut PdfDocument, opts: &AutotagOptions, ctx: &JobContext) -> Result<AutotagSummary> {
    let mut catalog = doc.catalog().map_err(pdf_err)?;
    if catalog.get_dict("StructTreeRoot").map_err(pdf_err)?.is_some() {
        return Err(Error::InvalidInput(
            "el documento ya tiene estructura de etiquetas; autoetiquetar la sustituiría".into(),
        ));
    }
    let mut pages = analyze(doc, ctx)?;
    // Negrita por (página, nombre de recurso de fuente).
    let mut bold = HashMap::new();
    for (pi, p) in pages.iter().enumerate() {
        let res = p.page.get_dict_inheritable("Resources").map_err(pdf_err)?;
        for it in &p.items {
            if let Some(f) = text_info(it).and_then(|t| t.font.clone()) {
                bold.entry((pi, f.clone())).or_insert_with(|| font_is_bold(&res, &f));
            }
        }
    }
    classify(&mut pages, &bold);

    // Raíz y elemento Document (el padre de los Document se fija tras crear la raíz).
    let mut root = doc.new_dict().map_err(pdf_err)?;
    root.dict_put("Type", new_name("StructTreeRoot")?).map_err(pdf_err)?;
    let root = doc.add_object(&root).map_err(pdf_err)?;
    let null = PdfObject::new_null();
    let mut document = make_elem(doc, "Document", &root, &null, None)?;

    let mut summary = AutotagSummary::default();
    let mut nums = doc.new_array().map_err(pdf_err)?;
    let total = pages.len() as u64 * 2;
    for (pi, pd) in pages.iter_mut().enumerate() {
        ctx.step(pages_len_done(pi, total), total, "Etiquetando")?;
        let blocks = build_blocks(pd);
        // 1) Asignar MCID en orden de aparición y escribir el contenido nuevo.
        let mut order: Vec<usize> = Vec::new(); // item → mcid por orden de bloques
        for b in &blocks {
            match b {
                Block::Heading(_, v) | Block::Para(v) => order.extend(v),
                Block::ListItem { label, body } => {
                    order.extend(label);
                    order.extend(body);
                }
                Block::Figure(i) => order.push(*i),
            }
        }
        let mut mcid_of: HashMap<usize, usize> = HashMap::new();
        let mut roles: HashMap<usize, &'static str> = HashMap::new();
        // El MCID sigue el orden del contenido, no el de los bloques.
        let mut tagged: Vec<usize> = order.clone();
        tagged.sort_unstable();
        for (m, i) in tagged.iter().enumerate() {
            mcid_of.insert(*i, m);
        }
        for b in &blocks {
            match b {
                Block::Heading(l, v) => {
                    let r = ["H1", "H2", "H3", "H4", "H5", "H6"][(*l as usize - 1).min(5)];
                    v.iter().for_each(|i| {
                        roles.insert(*i, r);
                    });
                }
                Block::Para(v) => v.iter().for_each(|i| {
                    roles.insert(*i, "P");
                }),
                Block::ListItem { label, body } => {
                    label.iter().for_each(|i| {
                        roles.insert(*i, "Lbl");
                    });
                    body.iter().for_each(|i| {
                        roles.insert(*i, "LBody");
                    });
                }
                Block::Figure(i) => {
                    roles.insert(*i, "Figure");
                }
            }
        }
        let mut out = Vec::with_capacity(pd.bytes.len() + pd.items.len() * 40);
        let mut cursor = 0;
        for (i, it) in pd.items.iter().enumerate() {
            if it.seg.start < cursor {
                continue;
            }
            out.extend(&pd.bytes[cursor..it.seg.start]);
            match (mcid_of.get(&i), roles.get(&i)) {
                (Some(m), Some(r)) => out.extend(format!("/{r} <</MCID {m}>> BDC\n").bytes()),
                _ => out.extend(b"/Artifact BMC\n"),
            }
            out.extend(&pd.bytes[it.seg.start..it.seg.end]);
            out.extend(b"\nEMC\n");
            cursor = it.seg.end;
        }
        out.extend(&pd.bytes[cursor..]);
        let stream = doc.add_stream(&bytes_buffer(&out)?, None, false).map_err(pdf_err)?;
        pd.page.dict_put("Contents", stream).map_err(pdf_err)?;
        pd.page
            .dict_put("StructParents", new_int(pi as i32)?)
            .map_err(pdf_err)?;

        // 2) Elementos de estructura y árbol de padres.
        let mut parents: Vec<Option<PdfObject>> = vec![None; tagged.len()];
        let page_ref = pd.page.try_clone().map_err(pdf_err)?;
        if xref_of(&page_ref) == 0 {
            return Err(Error::Pdf("no se pudo referenciar una página".into()));
        }
        let mut open_list: Option<PdfObject> = None;
        for b in &blocks {
            if !matches!(b, Block::ListItem { .. }) {
                open_list = None;
            }
            match b {
                Block::Heading(l, v) => {
                    let role = ["H1", "H2", "H3", "H4", "H5", "H6"][(*l as usize - 1).min(5)];
                    let e = leaf(doc, role, &document, &page_ref, v, &mcid_of, &mut parents)?;
                    push_kid(&mut document, e)?;
                    summary.headings += 1;
                }
                Block::Para(v) => {
                    let e = leaf(doc, "P", &document, &page_ref, v, &mcid_of, &mut parents)?;
                    push_kid(&mut document, e)?;
                    summary.paragraphs += 1;
                }
                Block::Figure(i) => {
                    let e = leaf(doc, "Figure", &document, &page_ref, &[*i], &mcid_of, &mut parents)?;
                    push_kid(&mut document, e)?;
                    summary.figures += 1;
                }
                Block::ListItem { label, body } => {
                    let mut list = match open_list.take() {
                        Some(l) => l,
                        None => {
                            let l = make_elem(doc, "L", &document, &page_ref, None)?;
                            push_kid(&mut document, l.try_clone().map_err(pdf_err)?)?;
                            summary.lists += 1;
                            l
                        }
                    };
                    let mut li = make_elem(doc, "LI", &list, &page_ref, None)?;
                    if let Some(l) = label {
                        let e = leaf(doc, "Lbl", &li, &page_ref, &[*l], &mcid_of, &mut parents)?;
                        push_kid(&mut li, e)?;
                    }
                    let e = leaf(doc, "LBody", &li, &page_ref, body, &mcid_of, &mut parents)?;
                    push_kid(&mut li, e)?;
                    push_kid(&mut list, li)?;
                    summary.list_items += 1;
                    open_list = Some(list);
                }
            }
        }
        let mut arr = doc.new_array().map_err(pdf_err)?;
        for p in parents {
            arr.array_push(p.unwrap_or_else(PdfObject::new_null)).map_err(pdf_err)?;
        }
        nums.array_push(new_int(pi as i32)?).map_err(pdf_err)?;
        nums.array_push(arr).map_err(pdf_err)?;
    }
    summary.artifacts = pages
        .iter()
        .flat_map(|p| &p.items)
        .filter(|i| i.class == Class::Artifact)
        .count();

    let mut root = root;
    let mut kids = doc.new_array().map_err(pdf_err)?;
    kids.array_push(document).map_err(pdf_err)?;
    root.dict_put("K", kids).map_err(pdf_err)?;
    let mut pt = doc.new_dict().map_err(pdf_err)?;
    pt.dict_put("Nums", nums).map_err(pdf_err)?;
    root.dict_put("ParentTree", pt).map_err(pdf_err)?;
    root.dict_put("ParentTreeNextKey", new_int(pages.len() as i32)?)
        .map_err(pdf_err)?;
    catalog.dict_put("StructTreeRoot", root).map_err(pdf_err)?;
    let mut mark = doc.new_dict().map_err(pdf_err)?;
    mark.dict_put("Marked", doc.new_bool(true)).map_err(pdf_err)?;
    catalog.dict_put("MarkInfo", mark).map_err(pdf_err)?;
    set_tab_order(doc)?;
    if let Some(l) = &opts.language {
        set_language(doc, l)?;
    }
    if let Some(t) = &opts.title {
        set_title(doc, t)?;
    }
    Ok(summary)
}

fn pages_len_done(pi: usize, total: u64) -> u64 {
    (total / 2 + pi as u64).min(total)
}

/// Elemento de estructura que contiene contenido marcado (uno o varios MCID).
fn leaf(
    doc: &mut PdfDocument,
    role: &str,
    parent: &PdfObject,
    page: &PdfObject,
    items: &[usize],
    mcid_of: &HashMap<usize, usize>,
    parents: &mut [Option<PdfObject>],
) -> Result<PdfObject> {
    let mcids: Vec<usize> = items.iter().filter_map(|i| mcid_of.get(i).copied()).collect();
    let k = if mcids.len() == 1 {
        new_int(mcids[0] as i32)?
    } else {
        let mut a = doc.new_array().map_err(pdf_err)?;
        for m in &mcids {
            a.array_push(new_int(*m as i32)?).map_err(pdf_err)?;
        }
        a
    };
    let e = make_elem(doc, role, parent, page, Some(k))?;
    for m in mcids {
        parents[m] = Some(e.try_clone().map_err(pdf_err)?);
    }
    Ok(e)
}

/// Etiqueta `input` y guarda el resultado en `output` (nunca sobrescribe la entrada).
pub fn autotag_file(input: &Path, output: &Path, opts: &AutotagOptions, ctx: &JobContext) -> Result<Report> {
    let mut doc = open_pdf(input, None)?;
    let s = autotag(&mut doc, opts, ctx)?;
    let mut options = PdfWriteOptions::default();
    options.set_garbage_level(1);
    options.set_compress(true);
    save_atomic(&doc, input, output, options)?;
    let mut report = Report::with_output(
        output,
        format!(
            "Etiquetado: {} encabezado(s), {} párrafo(s), {} lista(s) con {} elemento(s), {} figura(s); {} gráfico(s) marcados como decoración.",
            s.headings, s.paragraphs, s.lists, s.list_items, s.figures, s.artifacts
        ),
    );
    if s.figures > 0 {
        report.warnings.push(format!(
            "{} figura(s) necesitan texto alternativo: usa `pliego a11y check` para ver cuáles.",
            s.figures
        ));
    }
    report.warnings.push(
        "No se detectan tablas ni se etiquetan enlaces y campos: revisa el resultado con `pliego a11y check`.".into(),
    );
    Ok(report)
}
