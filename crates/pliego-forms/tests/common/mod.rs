//! Genera un PDF de prueba con un AcroForm (texto, casilla, radio, combo, lista) sin datos reales.
#![allow(dead_code)]

use pliego_pdf::PdfDocument;

pub fn form_pdf() -> Vec<u8> {
    let stream = |s: &str| {
        format!(
            "<< /Type /XObject /Subtype /Form /BBox [0 0 20 20] /Length {} >>\nstream\n{s}\nendstream",
            s.len()
        )
    };
    let on = stream("0 0 20 20 re S 4 4 m 16 16 l S 4 16 m 16 4 l S");
    let off = stream("0 0 20 20 re S");
    let objs: Vec<String> = vec![
        // 1 catálogo
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R 6 0 R 7 0 R 10 0 R 11 0 R] /DA (/Helv 0 Tf 0 g) /DR << /Font << /Helv 4 0 R >> >> /NeedAppearances true >> >>".into(),
        // 2 páginas
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        // 3 página
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /Helv 4 0 R >> >> /Annots [5 0 R 6 0 R 7 0 R 8 0 R 9 0 R 10 0 R 11 0 R] >>".into(),
        // 4 fuente
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
        // 5 texto
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (nombre) /Rect [50 700 250 720] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) >>".into(),
        // 6 casilla
        "<< /Type /Annot /Subtype /Widget /FT /Btn /T (acepto) /Rect [50 650 70 670] /F 4 /P 3 0 R /V /Off /AS /Off /AP << /N << /Yes 12 0 R /Off 13 0 R >> >> >>".into(),
        // 7 radio (campo padre)
        "<< /FT /Btn /Ff 32768 /T (sexo) /V /Off /Kids [8 0 R 9 0 R] >>".into(),
        // 8, 9 botones de radio
        "<< /Type /Annot /Subtype /Widget /Parent 7 0 R /Rect [50 600 70 620] /F 4 /P 3 0 R /AS /Off /AP << /N << /Mujer 12 0 R /Off 13 0 R >> >> >>".into(),
        "<< /Type /Annot /Subtype /Widget /Parent 7 0 R /Rect [90 600 110 620] /F 4 /P 3 0 R /AS /Off /AP << /N << /Hombre 12 0 R /Off 13 0 R >> >> >>".into(),
        // 10 combo
        "<< /Type /Annot /Subtype /Widget /FT /Ch /Ff 131072 /T (provincia) /Rect [50 550 250 570] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) /Opt [(Madrid) (Sevilla) (Vigo)] >>".into(),
        // 11 lista
        "<< /Type /Annot /Subtype /Widget /FT /Ch /T (idioma) /Rect [50 450 250 520] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) /Opt [(Español) (Gallego) (Catalán)] >>".into(),
        on,
        off,
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).into_bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).into_bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").into_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .into_bytes(),
    );
    out
}

pub fn open_form() -> PdfDocument {
    PdfDocument::from_bytes(&form_pdf()).unwrap()
}

pub fn reopen(doc: &PdfDocument) -> PdfDocument {
    let mut buf = Vec::new();
    doc.write_to(&mut buf).unwrap();
    PdfDocument::from_bytes(&buf).unwrap()
}
