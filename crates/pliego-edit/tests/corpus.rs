//! Verificación sobre el corpus de PDFs variados de `tests/corpus/edit/corpus`:
//! editar una palabra deja el resto de la página visualmente idéntico.

use pliego_document::{Document, SaveOptions};
use pliego_edit::*;
use pliego_pdf::{Colorspace, Matrix};
use std::path::PathBuf;

type Raster = (usize, usize, Vec<u8>);

fn dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/edit/corpus"))
}

fn render(doc: &Document) -> Raster {
    let page = doc.pdf().load_page(0).unwrap();
    let pm = page
        .to_pixmap(&Matrix::new_scale(1.0, 1.0), Colorspace::Rgb, false, true)
        .unwrap();
    (pm.width() as usize, pm.height() as usize, pm.samples().to_vec())
}

fn diff_outside(a: &Raster, b: &Raster, r: RectPts) -> (usize, [usize; 4]) {
    assert_eq!((a.0, a.1), (b.0, b.1));
    let mut n = 0;
    let mut bb = [usize::MAX, usize::MAX, 0, 0];
    for y in 0..a.1 {
        for x in 0..a.0 {
            let (fx, fy) = (x as f32, y as f32);
            if fx >= r[0] - 2.0 && fx <= r[2] + 2.0 && fy >= r[1] - 2.0 && fy <= r[3] + 2.0 {
                continue;
            }
            let i = (y * a.0 + x) * 3;
            if a.2[i..i + 3] != b.2[i..i + 3] {
                n += 1;
                bb = [bb[0].min(x), bb[1].min(y), bb[2].max(x), bb[3].max(y)];
            }
        }
    }
    (n, bb)
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn corpus_editar_una_palabra_no_toca_el_resto() {
    let mut files: Vec<_> = std::fs::read_dir(dir())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "pdf"))
        .collect();
    files.sort();
    assert!(files.len() >= 30, "corpus incompleto: {}", files.len());
    let tmp = tempfile::tempdir().unwrap();
    let mut fallos = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if let Err(e) = comprobar(path, tmp.path()) {
            fallos.push(format!("{name}: {e}"));
        }
    }
    assert!(fallos.is_empty(), "fallos en el corpus:\n{}", fallos.join("\n"));
}

fn comprobar(path: &std::path::Path, tmp: &std::path::Path) -> Result<(), String> {
    let mut doc = Document::open(path, None).map_err(|e| e.to_string())?;
    let blocks = text_blocks(&doc, 0).map_err(|e| e.to_string())?;
    // Bloque más grande con texto (el CJK sin fuente del sistema se omite).
    let Some(b) = blocks
        .iter()
        .filter(|b| b.text.split_whitespace().count() >= 2)
        .max_by_key(|b| b.text.len())
    else {
        return Err(format!("sin bloques de texto: {blocks:?}"));
    };
    let before = render(&doc);
    let text = flat(&b.text);
    let mut words: Vec<&str> = text.split(' ').collect();
    words[0] = "Ok";
    let nuevo = words.join(" ");
    let others: Vec<String> = blocks.iter().filter(|x| x.id != b.id).map(|x| flat(&x.text)).collect();

    match replace_text(&mut doc, 0, b.id, &nuevo, &Style::default()) {
        // Sin fuente CJK instalada en este equipo, el caso no se puede ejercitar: el texto se compone con recuadros
        // vacíos y se avisa.
        Ok(r) if r.warnings.iter().any(|w| w.contains("ninguna fuente instalada tiene")) => {
            eprintln!("{}: sin fuente instalada, se omite", path.display());
            return Ok(());
        }
        Ok(_) => {}
        Err(pliego_core::Error::Unsupported(m)) if m.contains("ninguna fuente") => {
            eprintln!("{}: sin fuente instalada, se omite", path.display());
            return Ok(());
        }
        Err(e) => return Err(format!("replace: {e}")),
    }
    let after = render(&doc);
    let (d, bb) = diff_outside(&before, &after, b.bbox);
    if d != 0 {
        return Err(format!(
            "{d} píxeles cambiados fuera del bloque {:?}; diff en {bb:?}",
            b.bbox
        ));
    }
    // El resto de bloques conserva su texto y el nuevo aparece.
    let now: Vec<String> = text_blocks(&doc, 0)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|x| flat(&x.text))
        .collect();
    for o in &others {
        if !now.contains(o) {
            return Err(format!("otro bloque alterado: {o:?} no está en {now:?}"));
        }
    }
    if !now.iter().any(|t| t.starts_with("Ok ") || t.contains(" Ok ")) {
        return Err(format!("el texto nuevo no aparece: {now:?}"));
    }
    // Guarda y reabre.
    let out = tmp.join(path.file_name().unwrap());
    doc.save(&out, &SaveOptions::default())
        .map_err(|e| format!("save: {e}"))?;
    let re = Document::open(&out, None).map_err(|e| format!("reabrir: {e}"))?;
    let again: Vec<String> = text_blocks(&re, 0)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|x| flat(&x.text))
        .collect();
    if !again.iter().any(|t| t.contains("Ok")) {
        return Err("tras reabrir falta el texto nuevo".into());
    }
    Ok(())
}
