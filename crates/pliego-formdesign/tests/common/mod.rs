//! PDF de prueba: dos páginas en blanco, sin formulario.
#![allow(dead_code)]

use pliego_pdf::PdfDocument;

pub fn blank_pdf() -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>",
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = vec![];
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).into_bytes());
    }
    let x = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).into_bytes());
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").into_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objs.len() + 1
        )
        .into_bytes(),
    );
    out
}

pub fn open_blank() -> PdfDocument {
    PdfDocument::from_bytes(&blank_pdf()).unwrap()
}

/// Guarda y vuelve a abrir, para comprobar lo que realmente queda en el archivo.
pub fn reopen(doc: &PdfDocument) -> PdfDocument {
    let mut buf = Vec::new();
    doc.write_to(&mut buf).unwrap();
    PdfDocument::from_bytes(&buf).unwrap()
}
