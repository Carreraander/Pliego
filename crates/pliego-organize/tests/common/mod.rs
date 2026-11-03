#![allow(dead_code)]
use std::path::{Path, PathBuf};

use mupdf::pdf::PdfDocument;
use mupdf::{Document, TextPageFlags};

/// PDF mínimo de `pages` páginas con el texto "<label> p<N>" y un marcador por página ("<label> m<N>").
pub fn build_pdf(label: &str, pages: usize) -> Vec<u8> {
    let mut objs: Vec<String> = Vec::new();
    // 1 = fuente; páginas: contenido (2+2i), página (3+2i); luego árbol, outlines y catálogo.
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    let pages_id = 2 + pages * 2;
    for i in 0..pages {
        let stream = format!("BT /F1 24 Tf 72 760 Td ({label} p{}) Tj ET", i + 1);
        objs.push(format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()));
        objs.push(format!(
            "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 1 0 R >> >> /Contents {} 0 R >>",
            2 + 2 * i
        ));
    }
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {pages} >>", kids.join(" ")));
    let outlines_id = objs.len() + 1;
    let first = outlines_id + 1;
    let last = outlines_id + pages;
    objs.push(format!(
        "<< /Type /Outlines /First {first} 0 R /Last {last} 0 R /Count {pages} >>"
    ));
    for i in 0..pages {
        let prev = if i > 0 {
            format!("/Prev {} 0 R", first + i - 1)
        } else {
            String::new()
        };
        let next = if i + 1 < pages {
            format!("/Next {} 0 R", first + i + 1)
        } else {
            String::new()
        };
        objs.push(format!(
            "<< /Title ({label} m{}) /Parent {outlines_id} 0 R {prev} {next} /Dest [{} 0 R /Fit] >>",
            i + 1,
            3 + 2 * i
        ));
    }
    let catalog = objs.len() + 1;
    objs.push(format!(
        "<< /Type /Catalog /Pages {pages_id} 0 R /Outlines {outlines_id} 0 R >>"
    ));
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for off in offsets {
        out.extend(format!("{off:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .bytes(),
    );
    out
}

pub fn write_pdf(dir: &Path, name: &str, label: &str, pages: usize) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, build_pdf(label, pages)).unwrap();
    p
}

pub fn open_pdf(path: &Path) -> PdfDocument {
    pliego_organize::open(path, None).unwrap()
}

pub fn page_text(doc: &PdfDocument, i: i32) -> String {
    let page = doc.load_page(i).unwrap();
    page.to_text_page(TextPageFlags::empty())
        .unwrap()
        .to_text()
        .unwrap()
        .trim()
        .to_string()
}

pub fn texts(doc: &PdfDocument) -> Vec<String> {
    (0..doc.page_count().unwrap()).map(|i| page_text(doc, i)).collect()
}

/// Para cada marcador de primer nivel: (título, texto de la página a la que apunta).
pub fn bookmark_targets(doc: &PdfDocument) -> Vec<(String, String)> {
    pliego_organize::read_outline(doc)
        .unwrap()
        .into_iter()
        .map(|o| (o.title, page_text(doc, o.page.expect("marcador sin destino") as i32)))
        .collect()
}

pub fn reopen(doc: &PdfDocument) -> PdfDocument {
    let mut buf = Vec::new();
    doc.write_to(&mut buf).unwrap();
    PdfDocument::from_bytes(&buf).unwrap()
}

pub fn _unused(_: &Document) {}
