use pliego_app_service::formdesign::detect_copy;
use pliego_core::JobContext;

/// PDF de una página con un texto.
fn pdf_with(text: &str) -> Vec<u8> {
    let s = format!("BT /F1 12 Tf 72 700 Td ({text}) Tj ET");
    let objs = [
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len()),
        "<< /Type /Page /Parent 4 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 1 0 R >> >> /Contents 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Catalog /Pages 4 0 R >>".to_string(),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, b) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj\n{b}\nendobj\n", i + 1).bytes());
    }
    let x = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for o in offs {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 5 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objs.len() + 1
        )
        .bytes(),
    );
    out
}

#[test]
fn detect_never_touches_the_original_and_only_writes_when_it_finds_fields() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("plano.pdf");
    let bytes = pdf_with("Nombre:");
    std::fs::write(&src, &bytes).unwrap();
    let report = detect_copy(&src, None, "campos", &JobContext::silent()).unwrap();
    assert_eq!(std::fs::read(&src).unwrap(), bytes);
    // Con campos: una copia nueva junto al original; sin campos: ninguna salida.
    match report.outputs.as_slice() {
        [] => assert_eq!(report.summary, "No se encontraron campos"),
        [out] => assert_eq!(out, &dir.path().join("plano (campos).pdf")),
        more => panic!("salidas inesperadas: {more:?}"),
    }
}

#[test]
fn missing_file_is_an_error() {
    let err = detect_copy(
        std::path::Path::new("no-existe.pdf"),
        None,
        "campos",
        &JobContext::silent(),
    );
    assert!(err.is_err());
}
