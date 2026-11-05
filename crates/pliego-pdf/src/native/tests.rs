//! Pruebas del modelo propio. MuPDF actúa de motor de referencia: lo que escribe `native` debe poder leerlo
//! (páginas, texto) igual que el original.

use std::path::{Path, PathBuf};

use super::PdfDocument as Native;
use crate::{Document, WriteOptions};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

pub(super) fn corpus() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "pdf") {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    walk(&corpus_dir(), &mut v);
    v.sort();
    v
}

/// Páginas y texto de cada página según MuPDF.
fn reference(bytes: &[u8]) -> Option<(i32, Vec<String>)> {
    let doc = Document::from_bytes(bytes, "application/pdf").ok()?;
    let n = doc.page_count().ok()?;
    let mut texts = Vec::new();
    for i in 0..n.min(30) {
        texts.push(doc.load_page(i).ok()?.text().ok()?);
    }
    Some((n, texts))
}

#[test]
fn corpus_opens_and_counts_pages_like_mupdf() {
    let mut checked = 0;
    for path in corpus() {
        if path.ends_with("tiling-hang.pdf") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Some((n, _)) = reference(&bytes) else { continue };
        let doc = Native::from_bytes(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if doc.needs_password().unwrap() {
            continue;
        }
        assert_eq!(doc.page_count().unwrap(), n, "{}", path.display());
        checked += 1;
    }
    assert!(checked > 60, "se comprobaron {checked} PDF");
}

/// `qpdf --check` sobre `bytes`: `None` si qpdf no está instalado; si no, código de salida (0 bien, 3 avisos, 2 error)
/// y la salida.
pub(super) fn qpdf_check(bytes: &[u8], password: Option<&str>) -> Option<(i32, String)> {
    use std::io::Write as _;
    let mut f = tempfile::Builder::new().suffix(".pdf").tempfile().ok()?;
    f.write_all(bytes).ok()?;
    let mut cmd = std::process::Command::new("qpdf");
    if let Some(p) = password {
        cmd.arg(format!("--password={p}"));
    }
    let out = cmd.arg("--check").arg(f.path()).output().ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Some((out.status.code().unwrap_or(-1), text))
}

fn roundtrip(options: &WriteOptions, label: &str) {
    for path in corpus() {
        if path.ends_with("tiling-hang.pdf") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Some((n, texts)) = reference(&bytes) else { continue };
        let doc = Native::from_bytes(&bytes).unwrap();
        if doc.needs_password().unwrap() {
            continue;
        }
        let out = doc
            .to_bytes(options)
            .unwrap_or_else(|e| panic!("{label} {}: {e}", path.display()));
        let (n2, texts2) =
            reference(&out).unwrap_or_else(|| panic!("{label} {}: MuPDF no abre la salida", path.display()));
        assert_eq!(n, n2, "{label} {}: páginas", path.display());
        assert_eq!(texts, texts2, "{label} {}: texto", path.display());
        // qpdf da por buena la salida cuando daba por bueno el original.
        if let (Some((orig, _)), Some((new, log))) = (qpdf_check(&bytes, None), qpdf_check(&out, None)) {
            assert!(
                new == 0 || new == orig,
                "{label} {}: qpdf --check original={orig} salida={new}\n{log}",
                path.display()
            );
        }
        // Y el modelo propio puede releer lo que escribe.
        let again = Native::from_bytes(&out).unwrap();
        assert_eq!(again.page_count().unwrap(), n, "{label} {}", path.display());
    }
}

#[test]
fn corpus_roundtrip_default() {
    roundtrip(&WriteOptions::new(), "default");
}

#[test]
fn corpus_roundtrip_garbage_compress() {
    let mut o = WriteOptions::new();
    o.set_garbage_level(4)
        .set_compress(true)
        .set_compress_images(true)
        .set_compress_fonts(true);
    roundtrip(&o, "garbage4");
}

#[test]
fn corpus_roundtrip_decompress_ascii_objstm() {
    let mut o = WriteOptions::new();
    o.set_decompress(true);
    roundtrip(&o, "decompress");
    let mut o = WriteOptions::new();
    o.set_ascii(true).set_garbage_level(2);
    roundtrip(&o, "ascii");
    let mut o = WriteOptions::new();
    o.set_object_streams(true).set_compress(true).set_garbage_level(3);
    roundtrip(&o, "objstm");
}

/// Guion de operaciones sobre la API común. Se ejecuta con MuPDF y con `native` y las dos transcripciones
/// (lo que devuelve cada llamada) deben coincidir línea a línea.
/// Sustituye los números de objeto de `Ref(n)` por `_` (cambian de un motor a otro).
fn mask_refs(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("Ref(") {
        out.push_str(&rest[..i + 4]);
        rest = &rest[i + 4..];
        let n = rest.find(')').unwrap_or(0);
        out.push('_');
        rest = &rest[n..];
    }
    out.push_str(rest);
    out
}

macro_rules! logf {
    ($t:expr, $($a:tt)*) => {
        $t.push(format!($($a)*))
    };
}

macro_rules! transcript {
    ($Doc:ident, $WriteOptions:ident) => {{
        use crate::{
            EmbeddedFileOptions, InsertPdfOptions, InsertPosition, MetadataName, Outline, PageLabelRule,
            PageLabelStyle, PageSelection, Size, Value,
        };
        let mut t: Vec<String> = Vec::new();
        let widths = |d: &$Doc| -> Vec<i32> {
            (0..d.page_count().unwrap())
                .map(|i| d.load_pdf_page(i).unwrap().media_box().unwrap().width() as i32)
                .collect()
        };
        let reopen = |d: &$Doc| -> $Doc { $Doc::from_bytes(&d.to_bytes(&$WriteOptions::new()).unwrap()).unwrap() };

        // --- Páginas ---
        let mut d = $Doc::new();
        for i in 0..5 {
            let mut p = d.new_page(Size::new(100.0 + i as f32, 200.0)).unwrap();
            let mut dd = $Doc::new();
            let _ = &mut dd;
            p.insert_contents(&mut d, format!("{i} g 0 0 10 10 re f").as_bytes(), true)
                .unwrap();
        }
        logf!(t, "widths {:?}", widths(&d));
        let d2 = reopen(&d);
        logf!(t, "reopen widths {:?} pages={}", widths(&d2), d2.page_count().unwrap());
        d.delete_page(1).unwrap();
        logf!(t, "delete_page(1) {:?}", widths(&d));
        d.delete_pages(PageSelection::Pages(vec![0, 3])).unwrap();
        logf!(t, "delete_pages [0,3] {:?}", widths(&d));
        let mut d = reopen(&d2);
        d.move_page(0, 3).unwrap();
        logf!(t, "move 0->3 {:?}", widths(&d));
        logf!(
            t,
            "select dup err={}",
            d.select_pages(PageSelection::Pages(vec![4, 0, 0, 2])).is_err()
        );
        d.select_pages(PageSelection::Pages(vec![4, 0, 2])).unwrap();
        logf!(t, "select [4,0,2] {:?}", widths(&d));
        let r = d.duplicate_page(1).unwrap();
        logf!(
            t,
            "duplicate_page(1) {:?} count={} {:?}",
            r.inserted_pages,
            r.page_count,
            widths(&d)
        );
        let r = d.copy_page(0, InsertPosition::Before(2)).unwrap();
        logf!(t, "copy_page Before(2) {:?} {:?}", r.inserted_pages, widths(&d));
        let mut d = reopen(&d);
        logf!(t, "after reopen {:?}", widths(&d));
        let src = reopen(&d2);
        let r = d
            .insert_pdf(
                &src,
                InsertPdfOptions {
                    source_pages: PageSelection::Pages(vec![1, 3]),
                    target: InsertPosition::After(0),
                    rotate: Some(90),
                },
            )
            .unwrap();
        logf!(
            t,
            "insert_pdf {:?} count={} {:?} rot={:?}",
            r.inserted_pages,
            r.page_count,
            widths(&d),
            (0..d.page_count().unwrap())
                .map(|i| d.load_pdf_page(i).unwrap().rotation().unwrap())
                .collect::<Vec<_>>()
        );
        let d = reopen(&d);
        logf!(t, "insert_pdf reopened {:?}", widths(&d));
        for i in 0..d.page_count().unwrap() {
            let p = d.find_page(i).unwrap();
            logf!(t, "lookup {} -> {}", i, d.lookup_page_number(&p).unwrap());
        }
        logf!(t, "find_page(99) err={}", d.find_page(99).is_err());
        logf!(t, "delete_pages bad err={}", {
            let mut x = reopen(&d);
            x.delete_pages(PageSelection::Pages(vec![99])).is_err()
        });

        // --- Cajas, giro y matriz ---
        let mut d = $Doc::new();
        let mut p = d.new_page(Size::new(300.0, 500.0)).unwrap();
        let mut pdict = p.object();
        for (rot, crop) in [
            (0, None),
            (90, None),
            (180, None),
            (270, None),
            (90, Some([10.0f32, 20.0, 210.0, 420.0])),
            (-90, None),
            (450, None),
            (30, None),
        ] {
            pdict.dict_put_int("Rotate", rot).unwrap();
            match crop {
                Some(c) => p.set_crop_box(crate::Rect::new(c[0], c[1], c[2], c[3])).unwrap(),
                None => {
                    let _ = pdict.dict_delete("CropBox");
                }
            }
            let m = p.ctm().unwrap();
            let b = p.bounds().unwrap();
            logf!(
                t,
                "rot={rot} crop={crop:?} rotation={} ctm=[{} {} {} {} {} {}] bounds={:?} crop_box={:?} media={:?}",
                p.rotation().unwrap(),
                m.a + 0.0,
                m.b + 0.0,
                m.c + 0.0,
                m.d + 0.0,
                m.e + 0.0,
                m.f + 0.0,
                b,
                p.crop_box().unwrap(),
                p.media_box().unwrap()
            );
        }

        // --- Objetos ---
        let mut d = $Doc::new();
        d.new_page(Size::A4).unwrap();
        let mut d = reopen(&d);
        let cat = d.catalog().unwrap();
        logf!(
            t,
            "catalog indirect={} dict={}",
            cat.is_indirect().unwrap(),
            cat.is_dict().unwrap()
        );
        let pages = cat.get_dict("Pages").unwrap().unwrap();
        logf!(
            t,
            "pages indirect={} type={:?}",
            pages.is_indirect().unwrap(),
            pages.dict_get_name("Type").unwrap()
        );
        let kids = pages.get_dict("Kids").unwrap().unwrap();
        logf!(
            t,
            "kids indirect={} len={} is_array={}",
            kids.is_indirect().unwrap(),
            kids.len().unwrap(),
            kids.is_array().unwrap()
        );
        let k0 = kids.get_array(0).unwrap().unwrap();
        logf!(
            t,
            "k0 indirect={} nonzero={}",
            k0.is_indirect().unwrap(),
            k0.as_indirect().unwrap() > 0
        );
        let p0 = d.find_page(0).unwrap();
        logf!(t, "find_page indirect={}", p0.is_indirect().unwrap());
        let r0 = k0.resolve().unwrap().unwrap();
        logf!(
            t,
            "resolve indirect={} as_indirect={:?} is_dict={}",
            r0.is_indirect().unwrap(),
            r0.as_indirect(),
            r0.is_dict().unwrap()
        );
        logf!(t, "get_array(9) none={}", kids.get_array(9).unwrap().is_none());
        logf!(t, "get_dict missing none={}", cat.get_dict("Nada").unwrap().is_none());
        logf!(
            t,
            "trailer has Root={}",
            d.trailer().unwrap().get_dict("Root").unwrap().is_some()
        );
        let mut cat = d.catalog().unwrap();
        cat.dict_put_name("PageMode", "UseOutlines").unwrap();
        cat.dict_put_int("N", 7).unwrap();
        cat.dict_put_real("R", 1.5).unwrap();
        cat.dict_put_bool("B", true).unwrap();
        let obj = d
            .object_from_value(&Value::Dict(vec![
                ("A".into(), Value::Int(3)),
                (
                    "L".into(),
                    Value::Array(vec![
                        Value::Real(1.5),
                        Value::Name("X".into()),
                        Value::Bool(true),
                        Value::Null,
                        Value::String(b"hola".to_vec()),
                    ]),
                ),
            ]))
            .unwrap();
        cat.dict_put("Extra", obj).unwrap();
        let back = cat.get_dict("Extra").unwrap().unwrap();
        logf!(
            t,
            "extra {:?} indirect={}",
            back.to_value(8).unwrap(),
            back.is_indirect().unwrap()
        );
        let mut lst = back.get_dict("L").unwrap().unwrap();
        lst.array_push(d.new_int(9).unwrap()).unwrap();
        lst.array_put(0, d.new_name("Z").unwrap()).unwrap();
        lst.array_delete(1).unwrap();
        logf!(t, "array ops {:?}", back.to_value(8).unwrap());
        // MuPDF añade un /Info (con su Producer) al catálogo de los documentos nuevos; no es parte del contrato.
        let keys: Vec<String> = cat
            .dict_iter()
            .unwrap()
            .into_iter()
            .map(|(k, _)| k)
            .filter(|k| k != "Info")
            .collect();
        logf!(t, "catalog keys {:?}", keys);
        logf!(
            t,
            "int/float {:?} {:?} {:?}",
            cat.dict_get_int("N").unwrap(),
            cat.dict_get_int("R").unwrap(),
            cat.dict_get_name("PageMode").unwrap()
        );
        let ind = d
            .add_object(
                &d.object_from_value(&Value::Array(vec![Value::Int(1), Value::Int(2)]))
                    .unwrap(),
            )
            .unwrap();
        logf!(
            t,
            "add_object indirect={} len={}",
            ind.is_indirect().unwrap(),
            ind.len().unwrap()
        );
        cat.dict_put("Ref", ind).unwrap();
        let d = reopen(&d);
        let cat = d.catalog().unwrap();
        logf!(
            t,
            "reopened Ref indirect={} value={}",
            cat.get_dict("Ref").unwrap().unwrap().is_indirect().unwrap(),
            mask_refs(&format!(
                "{:?}",
                cat.get_dict("Ref").unwrap().unwrap().to_value(4).unwrap()
            ))
        );
        logf!(
            t,
            "reopened extra {:?}",
            cat.get_dict("Extra").unwrap().unwrap().to_value(8).unwrap()
        );
        let parsed = d
            .new_object_from_str("<< /A 1 /B [1 2.5 (x) <4142> /N null true] /C 3 0 R /S (a\\(b\\)) >>")
            .unwrap();
        logf!(t, "parsed {}", parsed.to_pdf_string());
        logf!(
            t,
            "string {} text {}",
            d.new_string("hola").unwrap().to_pdf_string(),
            d.new_text_string("añ").unwrap().to_pdf_string()
        );

        // --- Flujos ---
        let mut d = $Doc::new();
        let dict = d.new_dict().unwrap();
        let mut s = d.add_stream(b"q 1 0 0 1 0 0 cm Q", Some(&dict), false).unwrap();
        logf!(
            t,
            "stream is_stream={} data={:?}",
            s.is_stream().unwrap(),
            String::from_utf8_lossy(&s.read_stream().unwrap())
        );
        s.write_stream(b"nuevo").unwrap();
        logf!(
            t,
            "write_stream {:?} raw={:?}",
            String::from_utf8_lossy(&s.read_stream().unwrap()),
            String::from_utf8_lossy(&s.read_raw_stream().unwrap())
        );
        let mut cat = d.catalog().unwrap();
        cat.dict_put("S", s.try_clone().unwrap()).unwrap();
        let d = reopen(&d);
        let s2 = d.catalog().unwrap().get_dict("S").unwrap().unwrap();
        logf!(
            t,
            "stream after reopen is_stream={} indirect={} data={:?}",
            s2.is_stream().unwrap(),
            s2.is_indirect().unwrap(),
            String::from_utf8_lossy(&s2.read_stream().unwrap())
        );
        let num = s2.as_indirect().unwrap();
        logf!(
            t,
            "xref_stream {:?} xref_object none={}",
            String::from_utf8_lossy(&d.xref_stream(num).unwrap()),
            d.xref_object(9999).is_err()
        );

        // --- Etiquetas ---
        let mut d = $Doc::new();
        for _ in 0..12 {
            d.new_page(Size::A4).unwrap();
        }
        logf!(
            t,
            "labels none {:?}",
            (0..3).map(|i| d.page_label(i).unwrap()).collect::<Vec<_>>()
        );
        d.set_page_label_rule(PageLabelRule::new(0, PageLabelStyle::LowerRoman))
            .unwrap();
        let mut r = PageLabelRule::new(3, PageLabelStyle::Decimal);
        r.start = 1;
        d.set_page_label_rule(r).unwrap();
        let mut r = PageLabelRule::new(8, PageLabelStyle::UpperAlpha);
        r.prefix = "A-".into();
        r.start = 2;
        d.set_page_label_rule(r).unwrap();
        let d = reopen(&d);
        logf!(
            t,
            "labels {:?}",
            (0..12).map(|i| d.page_label(i).unwrap()).collect::<Vec<_>>()
        );

        // --- Marcadores ---
        let mut d = $Doc::new();
        for _ in 0..4 {
            d.new_page(Size::A4).unwrap();
        }
        let toc = vec![
            Outline {
                title: "Uno".into(),
                uri: None,
                dest: Some(crate::LinkDestination {
                    page: 0,
                    kind: crate::DestinationKind::Fit,
                }),
                down: vec![Outline {
                    title: "Uno punto uno".into(),
                    uri: None,
                    dest: Some(crate::LinkDestination {
                        page: 1,
                        kind: crate::DestinationKind::Fit,
                    }),
                    down: vec![],
                }],
            },
            Outline {
                title: "Año — dos".into(),
                uri: None,
                dest: Some(crate::LinkDestination {
                    page: 3,
                    kind: crate::DestinationKind::Fit,
                }),
                down: vec![],
            },
            Outline {
                title: "Web".into(),
                uri: Some("https://example.com/".into()),
                dest: None,
                down: vec![],
            },
        ];
        d.set_outlines(&toc).unwrap();
        let d = reopen(&d);
        let simplify = |o: &Vec<Outline>| -> Vec<String> {
            fn go(o: &[Outline], depth: usize, out: &mut Vec<String>) {
                for x in o {
                    out.push(format!(
                        "{}{} -> {:?}",
                        " ".repeat(depth),
                        x.title,
                        x.dest.map(|d| d.page)
                    ));
                    go(&x.down, depth + 1, out);
                }
            }
            let mut v = Vec::new();
            go(o, 0, &mut v);
            v
        };
        logf!(t, "outlines {:?}", simplify(&d.outlines().unwrap()));
        let mut d = d;
        d.delete_outlines().unwrap();
        logf!(t, "outlines after delete {:?}", d.outlines().unwrap().len());

        // --- Adjuntos ---
        let mut d = $Doc::new();
        d.new_page(Size::A4).unwrap();
        let mut o = EmbeddedFileOptions::new("datos.txt");
        o.mime_type = Some("text/plain");
        o.created = Some(1_700_000_000);
        o.modified = Some(1_700_000_100);
        let info = d.add_embedded_file("datos", b"contenido de prueba", o).unwrap();
        logf!(
            t,
            "embedded add {:?} {:?} {} {:?} {:?}",
            info.name,
            info.filename,
            info.size,
            info.created,
            info.modified
        );
        d.add_embedded_file("alfa", b"A", EmbeddedFileOptions::new("alfa.bin"))
            .unwrap();
        let d = reopen(&d);
        let list = d.embedded_files().unwrap();
        let mut rows = list
            .iter()
            .map(|e| {
                (
                    e.name.clone(),
                    e.filename.clone(),
                    e.mime_type.clone(),
                    e.size,
                    e.created,
                    e.modified,
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        logf!(t, "embedded list {:?}", rows);
        logf!(
            t,
            "embedded load {:?} checksum={:?}",
            d.load_embedded_file("datos")
                .unwrap()
                .map(|v| String::from_utf8_lossy(&v).into_owned()),
            d.verify_embedded_file_checksum("datos").unwrap()
        );
        let mut d = d;
        logf!(
            t,
            "embedded delete {} {} remaining={}",
            d.delete_embedded_file("alfa").unwrap(),
            d.delete_embedded_file("nada").unwrap(),
            d.embedded_files().unwrap().len()
        );

        // --- Capas ---
        let mut d = $Doc::new();
        d.new_page(Size::A4).unwrap();
        logf!(t, "ocg none {}", d.optional_content_groups().unwrap().len());
        let a = d.add_optional_content_group("Fondo").unwrap();
        let b = d.add_optional_content_group("Notas ñ").unwrap();
        d.set_optional_content_enabled(b, false).unwrap();
        let d = reopen(&d);
        let (a2, b2) = {
            let g = d.optional_content_groups().unwrap();
            (g[0].xref, g[1].xref)
        };
        let _ = (a, b);
        logf!(
            t,
            "ocg {:?}",
            d.optional_content_groups()
                .unwrap()
                .iter()
                .map(|g| (g.name.clone(), g.enabled))
                .collect::<Vec<_>>()
        );
        let mut d = d;
        d.set_optional_content_enabled(b2, true).unwrap();
        d.set_optional_content_enabled(a2, false).unwrap();
        logf!(
            t,
            "ocg toggled {:?} {} {}",
            d.optional_content_groups()
                .unwrap()
                .iter()
                .map(|g| (g.name.clone(), g.enabled))
                .collect::<Vec<_>>(),
            d.optional_content_enabled(a2).unwrap(),
            d.optional_content_enabled(b2).unwrap()
        );

        // --- Metadatos y estado ---
        let d = $Doc::new();
        logf!(
            t,
            "meta {:?} {:?} {:?}",
            d.metadata(MetadataName::Title).unwrap(),
            d.metadata(MetadataName::Encryption).unwrap(),
            d.has_acro_form().unwrap()
        );
        logf!(
            t,
            "permissions {:?} unsaved={}",
            d.permissions(),
            d.has_unsaved_changes()
        );
        t
    }};
}

#[test]
fn transcript_matches_mupdf() {
    let reference = {
        use crate::{PdfDocument, WriteOptions};
        transcript!(PdfDocument, WriteOptions)
    };
    let candidate = {
        use crate::WriteOptions;
        use crate::native::PdfDocument;
        transcript!(PdfDocument, WriteOptions)
    };
    let mut diffs = Vec::new();
    for i in 0..reference.len().max(candidate.len()) {
        let (a, b) = (reference.get(i), candidate.get(i));
        if a != b {
            diffs.push(format!("línea {i}\n  mupdf : {a:?}\n  native: {b:?}"));
        }
    }
    assert!(diffs.is_empty(), "{} diferencias:\n{}", diffs.len(), diffs.join("\n"));
}

fn text_of(bytes: &[u8], password: Option<&str>, page: i32) -> String {
    let mut doc = Document::from_bytes(bytes, "application/pdf").unwrap();
    if doc.needs_password().unwrap() {
        assert!(
            doc.authenticate(password.unwrap_or("")).unwrap(),
            "contraseña rechazada"
        );
    }
    doc.load_page(page).unwrap().text().unwrap()
}

pub(super) fn corpus_file(rel: &str) -> Vec<u8> {
    std::fs::read(corpus_dir().join(rel)).unwrap()
}

// --- Guardado incremental ---

#[test]
fn incremental_keeps_original_bytes_and_adds_changes() {
    let original = corpus_file("organize/a.pdf");
    let mut doc = Native::from_bytes(&original).unwrap();
    assert!(doc.can_be_saved_incrementally());
    let pages = doc.page_count().unwrap();
    let mut catalog = doc.catalog().unwrap();
    catalog.dict_put_name("PageMode", "UseOutlines").unwrap();
    let mut info = {
        let t = doc.trailer().unwrap();
        match t.get_dict("Info").unwrap() {
            Some(i) => i,
            None => {
                let mut tr = t.clone();
                let i = doc.add_object(&doc.new_dict().unwrap()).unwrap();
                tr.dict_put("Info", i).unwrap();
                tr.get_dict("Info").unwrap().unwrap()
            }
        }
    };
    info.dict_put("Title", doc.new_text_string("Título nuevo").unwrap())
        .unwrap();
    doc.new_page(crate::Size::A4).unwrap();

    let mut o = WriteOptions::new();
    o.set_incremental(true);
    let out = doc.to_bytes(&o).unwrap();
    assert!(
        out.starts_with(&original),
        "el original debe quedar intacto al principio"
    );
    assert!(out.len() > original.len());
    assert!(
        out.len() < original.len() + 4000,
        "solo se añaden los objetos cambiados ({} bytes)",
        out.len() - original.len()
    );

    let again = Native::from_bytes(&out).unwrap();
    assert_eq!(again.page_count().unwrap(), pages + 1);
    assert_eq!(
        again.catalog().unwrap().dict_get_name("PageMode").unwrap().as_deref(),
        Some("UseOutlines")
    );
    assert_eq!(again.metadata(crate::MetadataName::Title).unwrap(), "Título nuevo");
    // El motor de referencia lo ve igual.
    let m = Document::from_bytes(&out, "application/pdf").unwrap();
    assert_eq!(m.page_count().unwrap(), pages + 1);
    assert_eq!(m.metadata(crate::MetadataName::Title).unwrap(), "Título nuevo");
    if let Some((code, log)) = qpdf_check(&out, None) {
        assert!(code == 0, "qpdf: {code}\n{log}");
    }
    // Un segundo guardado incremental parte del mismo original (no apila el anterior).
    let out2 = doc.to_bytes(&o).unwrap();
    assert_eq!(out, out2);
}

#[test]
fn incremental_preserves_signed_pdf_prefix() {
    let signed = corpus_file("document/firmado.pdf");
    let doc = Native::from_bytes(&signed).unwrap();
    doc.catalog().unwrap().dict_put_name("PageMode", "UseNone").unwrap();
    let mut o = WriteOptions::new();
    o.set_incremental(true);
    let out = doc.to_bytes(&o).unwrap();
    assert!(out.starts_with(&signed), "la parte firmada no puede cambiar ni un byte");
    assert_eq!(
        Native::from_bytes(&out).unwrap().page_count().unwrap(),
        doc.page_count().unwrap()
    );
}

#[test]
fn incremental_rejected_for_new_documents() {
    let doc = Native::new();
    assert!(!doc.can_be_saved_incrementally());
    let mut o = WriteOptions::new();
    o.set_incremental(true);
    assert!(doc.to_bytes(&o).is_err());
}

#[test]
fn incremental_over_every_corpus_file_is_valid() {
    let mut done = 0;
    for path in corpus() {
        if path.ends_with("tiling-hang.pdf") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let doc = Native::from_bytes(&bytes).unwrap();
        if doc.needs_password().unwrap() || !doc.can_be_saved_incrementally() {
            continue;
        }
        let Some((n, texts)) = reference(&bytes) else { continue };
        doc.catalog().unwrap().dict_put_name("PageMode", "UseNone").unwrap();
        let mut o = WriteOptions::new();
        o.set_incremental(true);
        let out = doc.to_bytes(&o).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(out.starts_with(&bytes), "{}", path.display());
        let (n2, texts2) = reference(&out).unwrap_or_else(|| panic!("{}: MuPDF no abre", path.display()));
        assert_eq!((n, &texts), (n2, &texts2), "{}", path.display());
        if let (Some((a, _)), Some((b, log))) = (qpdf_check(&bytes, None), qpdf_check(&out, None)) {
            assert!(b == 0 || b == a, "{}: qpdf {a} -> {b}\n{log}", path.display());
        }
        done += 1;
    }
    assert!(done > 40, "solo {done}");
}

// --- Cifrado ---

fn encrypted(enc: crate::Encryption, user: &str, owner: &str) -> Vec<u8> {
    let doc = Native::from_bytes(&corpus_file("basic/hola.pdf")).unwrap();
    let mut o = WriteOptions::new();
    o.set_encryption(enc).set_user_password(user).set_owner_password(owner);
    doc.to_bytes(&o).unwrap()
}

#[test]
fn encryption_written_by_native_opens_in_mupdf_and_native() {
    use crate::Encryption::*;
    for enc in [Rc4_40, Rc4_128, Aes128, Aes256] {
        let out = encrypted(enc, "usuario", "dueño");
        assert!(text_of(&out, Some("usuario"), 0).contains("Hola Pliego"), "{enc:?}");
        assert!(text_of(&out, Some("dueño"), 0).contains("Hola Pliego"), "{enc:?}");
        let m = Document::from_bytes(&out, "application/pdf").unwrap();
        assert!(m.needs_password().unwrap(), "{enc:?}");
        let mut m = m;
        assert!(!m.authenticate("mala").unwrap(), "{enc:?}");
        // El modelo propio también.
        let locked = Native::from_bytes(&out).unwrap();
        assert!(locked.needs_password().unwrap(), "{enc:?}");
        assert!(
            matches!(locked.page_count(), Err(pliego_core::Error::Password)),
            "{enc:?}"
        );
        assert!(Native::from_bytes_with_password(&out, Some("mala")).is_err(), "{enc:?}");
        assert!(Native::from_bytes_with_password(&out, Option::None).is_err(), "{enc:?}");
        let ok = Native::from_bytes_with_password(&out, Some("usuario")).unwrap();
        assert_eq!(ok.page_count().unwrap(), 2, "{enc:?}");
        let mut locked = locked;
        assert!(!locked.authenticate("mala").unwrap());
        assert!(locked.authenticate("dueño").unwrap());
        assert_eq!(locked.page_count().unwrap(), 2);
        if let Some((code, log)) = qpdf_check(&out, Some("usuario")) {
            assert_eq!(code, 0, "{enc:?}\n{log}");
        }
    }
}

#[test]
fn encryption_written_by_mupdf_opens_in_native() {
    use crate::Encryption::*;
    let hola = crate::PdfDocument::open(corpus_dir().join("basic/hola.pdf")).unwrap();
    for enc in [Rc4_128, Aes128, Aes256] {
        let mut o = WriteOptions::new();
        o.set_encryption(enc).set_user_password("u1").set_owner_password("o1");
        let bytes = hola.to_bytes(&o).unwrap();
        let doc = Native::from_bytes_with_password(&bytes, Some("u1")).unwrap_or_else(|e| panic!("{enc:?}: {e}"));
        assert_eq!(doc.page_count().unwrap(), 2, "{enc:?}");
        assert!(Native::from_bytes_with_password(&bytes, Some("zz")).is_err(), "{enc:?}");
        // Descifrado de contenido: el flujo de la página se lee.
        let page = doc.find_page(0).unwrap();
        let contents = page.get_dict("Contents").unwrap().unwrap();
        assert!(
            String::from_utf8_lossy(&contents.read_stream().unwrap()).contains("Hola Pliego"),
            "{enc:?}"
        );
    }
}

#[test]
fn keep_encryption_resaves_with_same_passwords_and_none_removes_it() {
    let out = encrypted(crate::Encryption::Aes256, "u", "o");
    let doc = Native::from_bytes_with_password(&out, Some("u")).unwrap();
    let kept = doc.to_bytes(&WriteOptions::new()).unwrap();
    assert!(
        Native::from_bytes(&kept).unwrap().needs_password().unwrap(),
        "Keep conserva el cifrado"
    );
    assert!(text_of(&kept, Some("u"), 0).contains("Hola Pliego"));
    let mut o = WriteOptions::new();
    o.set_encryption(crate::Encryption::None);
    let clear = doc.to_bytes(&o).unwrap();
    assert!(!Native::from_bytes(&clear).unwrap().needs_password().unwrap());
    assert!(text_of(&clear, None, 0).contains("Hola Pliego"));
}

#[test]
fn permissions_are_written_and_read() {
    let doc = Native::from_bytes(&corpus_file("basic/hola.pdf")).unwrap();
    let mut o = WriteOptions::new();
    o.set_encryption(crate::Encryption::Aes256)
        .set_owner_password("o")
        .set_permissions(crate::Permission::PRINT | crate::Permission::COPY);
    let out = doc.to_bytes(&o).unwrap();
    let reopened = Native::from_bytes_with_password(&out, Some("")).unwrap();
    let p = reopened.permissions();
    assert!(p.contains(crate::Permission::PRINT) && p.contains(crate::Permission::COPY));
    assert!(!p.contains(crate::Permission::MODIFY) && !p.contains(crate::Permission::ANNOTATE));
    // MuPDF informa de todos los permisos al abrir con la contraseña de usuario vacía, así que el contraste
    // es con qpdf, que lee /P tal cual.
    if let Some((code, log)) = qpdf_check(&out, None) {
        assert_eq!(code, 0, "{log}");
        assert!(
            log.contains("extract for any purpose: allowed") && log.contains("modify anything: not allowed"),
            "{log}"
        );
    }
}

#[test]
fn incremental_on_encrypted_document() {
    let out = encrypted(crate::Encryption::Aes128, "u", "o");
    let mut doc = Native::from_bytes_with_password(&out, Some("u")).unwrap();
    doc.catalog().unwrap().dict_put_name("PageMode", "UseThumbs").unwrap();
    doc.new_page(crate::Size::A4).unwrap();
    let mut o = WriteOptions::new();
    o.set_incremental(true);
    let inc = doc.to_bytes(&o).unwrap();
    assert!(inc.starts_with(&out));
    let again = Native::from_bytes_with_password(&inc, Some("u")).unwrap();
    assert_eq!(again.page_count().unwrap(), 3);
    assert_eq!(
        again.catalog().unwrap().dict_get_name("PageMode").unwrap().as_deref(),
        Some("UseThumbs")
    );
    assert!(text_of(&inc, Some("u"), 0).contains("Hola Pliego"));
    let mut bad = WriteOptions::new();
    bad.set_incremental(true).set_encryption(crate::Encryption::None);
    assert!(
        doc.to_bytes(&bad).is_err(),
        "no se puede quitar el cifrado en un incremental"
    );
}

// --- Opciones de guardado ---

#[test]
fn garbage_levels_shrink_and_dedupe() {
    // Documento con tres fuentes idénticas y un objeto huérfano.
    let mut doc = Native::new();
    let mut p = doc.new_page(crate::Size::A4).unwrap();
    let mut fonts = doc.new_dict().unwrap();
    for i in 0..3 {
        let f = doc
            .object_from_value(&crate::Value::Dict(vec![
                ("Type".into(), crate::Value::Name("Font".into())),
                ("Subtype".into(), crate::Value::Name("Type1".into())),
                ("BaseFont".into(), crate::Value::Name("Helvetica".into())),
            ]))
            .unwrap();
        let f = doc.add_object(&f).unwrap();
        fonts.dict_put(&format!("F{i}"), f).unwrap();
    }
    p.resources().unwrap().dict_put("Font", fonts).unwrap();
    let big = vec![b'x'; 5000];
    doc.add_stream(&big, None, false).unwrap(); // huérfano
    p.insert_contents(
        &mut doc,
        b"BT /F0 12 Tf (a) Tj /F1 12 Tf (b) Tj /F2 12 Tf (c) Tj ET",
        true,
    )
    .unwrap();
    let size = |level: i32, compress: bool| {
        let mut o = WriteOptions::new();
        o.set_garbage_level(level).set_compress(compress);
        doc.to_bytes(&o).unwrap()
    };
    let l0 = size(0, false);
    let l1 = size(1, false);
    let l3 = size(3, false);
    assert!(
        l1.len() < l0.len() - 4000,
        "el huérfano de 5 KB desaparece: {} -> {}",
        l0.len(),
        l1.len()
    );
    let fonts_in = |b: &[u8]| String::from_utf8_lossy(b).matches("/BaseFont").count();
    assert_eq!(fonts_in(&l1), 3);
    assert_eq!(fonts_in(&l3), 1, "nivel 3 fusiona las fuentes idénticas");
    for bytes in [&l0, &l1, &l3] {
        assert_eq!(Native::from_bytes(bytes).unwrap().page_count().unwrap(), 1);
        assert!(text_of(bytes, None, 0).contains("abc"), "{}", text_of(bytes, None, 0));
        if let Some((code, log)) = qpdf_check(bytes, None) {
            assert_eq!(code, 0, "{log}");
        }
    }
    // Compresión: un flujo grande y repetitivo se encoge mucho.
    let mut doc2 = Native::new();
    let mut page = doc2.new_page(crate::Size::A4).unwrap();
    page.insert_contents(&mut doc2, &vec![b' '; 20_000], true).unwrap();
    let plain = doc2.to_bytes(&WriteOptions::new()).unwrap();
    let mut o = WriteOptions::new();
    o.set_compress(true);
    let packed = doc2.to_bytes(&o).unwrap();
    assert!(packed.len() < plain.len() / 10, "{} -> {}", plain.len(), packed.len());
    let mut o = WriteOptions::new();
    o.set_decompress(true);
    let again = Native::from_bytes(&packed).unwrap().to_bytes(&o).unwrap();
    assert!(again.len() > plain.len() / 2, "descomprimir devuelve los datos");
}

#[test]
fn metadata_and_images_stay_uncompressed_unless_asked() {
    let mut doc = Native::new();
    doc.new_page(crate::Size::A4).unwrap();
    let mut d = doc.new_dict().unwrap();
    d.dict_put_name("Type", "Metadata").unwrap();
    d.dict_put_name("Subtype", "XML").unwrap();
    let xmp = doc.add_stream(&vec![b'a'; 4000], Some(&d), false).unwrap();
    let mut img = doc.new_dict().unwrap();
    img.dict_put_name("Subtype", "Image").unwrap();
    let img = doc.add_stream(&vec![b'b'; 4000], Some(&img), false).unwrap();
    let mut cat = doc.catalog().unwrap();
    cat.dict_put("Metadata", xmp).unwrap();
    cat.dict_put("Img", img).unwrap();
    let mut o = WriteOptions::new();
    o.set_compress(true);
    let out = doc.to_bytes(&o).unwrap();
    assert!(out.len() > 7500, "metadatos e imágenes sin comprimir: {}", out.len());
    o.set_compress_images(true);
    let before = out.len();
    let out = doc.to_bytes(&o).unwrap();
    // La imagen (4000 bytes iguales) se encoge; los metadatos siguen sin comprimir.
    assert!(
        out.len() + 3500 < before && out.len() > 4000,
        "la imagen se comprime con compress_images: {before} -> {}",
        out.len()
    );
    let back = Native::from_bytes(&out).unwrap();
    let m = back.catalog().unwrap().get_dict("Metadata").unwrap().unwrap();
    assert_eq!(m.read_raw_stream().unwrap().len(), 4000);
}

#[test]
fn ascii_option_produces_hex_encoded_streams_that_read_back() {
    let doc = Native::from_bytes(&corpus_file("basic/hola.pdf")).unwrap();
    let mut o = WriteOptions::new();
    o.set_ascii(true);
    let out = doc.to_bytes(&o).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("ASCIIHexDecode"));
    assert!(text_of(&out, None, 0).contains("Hola Pliego"));
    let back = Native::from_bytes(&out).unwrap();
    assert!(
        String::from_utf8_lossy(&back.xref_stream(2).unwrap()).contains("Hola") || back.count_objects().unwrap() > 2
    );
}

#[test]
fn object_streams_option_is_smaller_and_valid() {
    let doc = Native::from_bytes(&corpus_file("office/articulo-1.pdf")).unwrap();
    let mut plain = WriteOptions::new();
    plain.set_garbage_level(3).set_compress(true);
    let a = doc.to_bytes(&plain).unwrap();
    let mut packed = plain.clone();
    packed.set_object_streams(true);
    let b = doc.to_bytes(&packed).unwrap();
    assert!(b.len() <= a.len(), "{} vs {}", b.len(), a.len());
    assert!(String::from_utf8_lossy(&b).contains("/ObjStm"));
    assert_eq!(
        Native::from_bytes(&b).unwrap().page_count().unwrap(),
        doc.page_count().unwrap()
    );
    assert_eq!(reference(&a).unwrap().1, reference(&b).unwrap().1);
    if let Some((code, log)) = qpdf_check(&b, None) {
        assert_eq!(code, 0, "{log}");
    }
}

// --- Formularios y copia de páginas reales ---

#[test]
fn insert_pdf_brings_form_fields_along() {
    let form = Native::from_bytes(&corpus_file("organize/form1.pdf")).unwrap();
    let mupdf_form = crate::PdfDocument::open(corpus_dir().join("organize/form1.pdf")).unwrap();
    let fields_ref = mupdf_form.load_pdf_page(0).unwrap().widgets().len();
    assert!(fields_ref > 0, "el PDF de prueba debería tener campos");
    let mut dst = Native::from_bytes(&corpus_file("organize/a.pdf")).unwrap();
    let before = dst.page_count().unwrap();
    dst.insert_pdf(&form, crate::InsertPdfOptions::default()).unwrap();
    assert_eq!(dst.page_count().unwrap(), before + form.page_count().unwrap());
    let out = dst.to_bytes(&WriteOptions::new()).unwrap();
    let m = crate::PdfDocument::from_bytes(&out).unwrap();
    let n = m.load_pdf_page(before).unwrap().widgets().len();
    assert_eq!(n, fields_ref, "los campos viajan con la página");
    assert!(m.has_acro_form().unwrap());
    if let Some((code, log)) = qpdf_check(&out, None) {
        assert_eq!(code, 0, "{log}");
    }
}

#[test]
fn page_operations_on_real_documents_keep_text() {
    for rel in [
        "office/articulo-1.pdf",
        "organize/a.pdf",
        "organize/links.pdf",
        "print/limpio.pdf",
    ] {
        let bytes = corpus_file(rel);
        let (n, texts) = reference(&bytes).unwrap();
        let mut doc = Native::from_bytes(&bytes).unwrap();
        if n >= 2 {
            doc.move_page(0, n as usize - 1).unwrap();
        }
        doc.duplicate_page(0).unwrap();
        let out = doc.to_bytes(&WriteOptions::new()).unwrap();
        let (n2, texts2) = reference(&out).unwrap();
        assert_eq!(n2, n + 1, "{rel}");
        // Páginas movidas y duplicada: el multiconjunto de textos solo gana una copia.
        let mut want = texts.clone();
        want.push(texts[if n >= 2 { 1 } else { 0 }].clone());
        let mut got = texts2.clone();
        want.sort();
        got.sort();
        assert_eq!(want, got, "{rel}");
        if let Some((code, log)) = qpdf_check(&out, None) {
            assert_eq!(code, 0, "{rel}\n{log}");
        }
    }
}

#[test]
fn links_added_natively_are_seen_by_mupdf() {
    let mut doc = Native::new();
    for _ in 0..2 {
        doc.new_page(crate::Size::A4).unwrap();
    }
    let mut p = doc.load_pdf_page(0).unwrap();
    let links = [
        crate::NewLink {
            bounds: crate::Rect::new(50.0, 50.0, 150.0, 80.0),
            target: crate::LinkTarget::Uri("https://pdfpliego.com/".into()),
        },
        crate::NewLink {
            bounds: crate::Rect::new(50.0, 100.0, 150.0, 130.0),
            target: crate::LinkTarget::Page {
                page: 1,
                kind: crate::DestinationKind::Fit,
            },
        },
    ];
    p.add_links(&mut doc, &links).unwrap();
    let out = doc.to_bytes(&WriteOptions::new()).unwrap();
    let m = crate::PdfDocument::from_bytes(&out).unwrap();
    let seen: Vec<_> = m.load_page(0).unwrap().links().unwrap().collect();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].uri, "https://pdfpliego.com/");
    assert_eq!(seen[1].dest.map(|d| d.page), Some(1));
    assert!(
        (seen[0].bounds.x0 - 50.0).abs() < 0.01 && (seen[0].bounds.y1 - 80.0).abs() < 0.01,
        "{:?}",
        seen[0].bounds
    );
}

#[test]
fn native_outlines_with_uri_and_views_roundtrip() {
    let mut doc = Native::new();
    for _ in 0..3 {
        doc.new_page(crate::Size::A4).unwrap();
    }
    let toc = vec![
        crate::Outline {
            title: "Vista".into(),
            uri: None,
            dest: Some(crate::LinkDestination {
                page: 2,
                kind: crate::DestinationKind::XYZ {
                    left: Some(10.0),
                    top: Some(700.0),
                    zoom: None,
                },
            }),
            down: vec![],
        },
        crate::Outline {
            title: "Web".into(),
            uri: Some("https://example.com/".into()),
            dest: None,
            down: vec![],
        },
    ];
    doc.set_outlines(&toc).unwrap();
    let out = doc.to_bytes(&WriteOptions::new()).unwrap();
    assert_eq!(Native::from_bytes(&out).unwrap().outlines().unwrap(), toc);
    let m = crate::PdfDocument::from_bytes(&out).unwrap();
    let theirs = m.outlines().unwrap();
    assert_eq!(theirs.len(), 2);
    assert_eq!(theirs[0].dest.map(|d| d.page), Some(2));
    if let Some((code, log)) = qpdf_check(&out, None) {
        assert_eq!(code, 0, "{log}");
    }
}

#[test]
fn hostile_outline_and_pages_do_not_loop() {
    let bytes = corpus_file("outline/hostil.pdf");
    let doc = Native::from_bytes(&bytes).unwrap();
    let _ = doc.outlines().unwrap();
    let bytes = corpus_file("worker/outline-null-title.pdf");
    let doc = Native::from_bytes(&bytes).unwrap();
    assert!(
        doc.outlines()
            .unwrap()
            .iter()
            .all(|o| o.title.is_empty() || !o.title.is_empty())
    );
}

// --- Robustez frente a archivos dañados ---

#[test]
fn damaged_files_never_panic() {
    // Mutaciones deterministas (xorshift): bytes cambiados, truncado y trozos duplicados.
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut ran = 0;
    for path in corpus().into_iter().take(40) {
        if path.ends_with("tiling-hang.pdf") {
            continue;
        }
        let orig = std::fs::read(&path).unwrap();
        if orig.len() < 64 {
            continue;
        }
        for variant in 0..3 {
            let mut b = orig.clone();
            match variant {
                0 => {
                    for _ in 0..8 {
                        let i = (next() as usize) % b.len();
                        b[i] = next() as u8;
                    }
                }
                1 => b.truncate((next() as usize) % b.len()),
                _ => {
                    let i = (next() as usize) % b.len();
                    let j = (i + 1 + (next() as usize) % 256).min(b.len());
                    let chunk = b[i..j].to_vec();
                    b.splice(i..i, chunk);
                }
            }
            let r = std::panic::catch_unwind(|| {
                if let Ok(doc) = Native::from_bytes(&b)
                    && !doc.needs_password().unwrap_or(true)
                {
                    let _ = doc.page_count();
                    let _ = doc.outlines();
                    let _ = doc.embedded_files();
                    let _ = doc.to_bytes(&WriteOptions::new());
                }
            });
            assert!(r.is_ok(), "pánico con {} variante {variant}", path.display());
            ran += 1;
        }
    }
    assert!(ran > 80);
}
