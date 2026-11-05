use pliego_app_service::actions::{BatchRequest, run_batch};
use pliego_core::JobContext;

/// PDF mínimo de `pages` páginas A4 con el texto "P<N>".
fn build_pdf(pages: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec!["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into()];
    let pages_id = 2 + pages * 2;
    for i in 0..pages {
        let s = format!("BT /F1 24 Tf 250 400 Td (P{}) Tj ET", i + 1);
        objs.push(format!("<< /Length {} >>\nstream\n{s}\nendstream", s.len()));
        objs.push(format!(
            "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 1 0 R >> >> /Contents {} 0 R >>",
            2 + 2 * i
        ));
    }
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {pages} >>", kids.join(" ")));
    objs.push(format!("<< /Type /Catalog /Pages {pages_id} 0 R >>"));
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
            "trailer\n<< /Size {} /Root {} 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objs.len() + 1,
            objs.len()
        )
        .bytes(),
    );
    out
}

#[test]
fn runs_an_action_over_a_folder_without_touching_the_originals() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("docs");
    std::fs::create_dir(&docs).unwrap();
    std::fs::write(docs.join("a.pdf"), build_pdf(3)).unwrap();
    std::fs::write(docs.join("b.pdf"), build_pdf(2)).unwrap();
    let action = dir.path().join("accion.toml");
    std::fs::write(&action, "[[steps]]\nkind = \"pages.delete\"\npages = \"1\"\n").unwrap();

    let report = run_batch(
        &BatchRequest {
            action,
            inputs: vec![docs.clone()],
            suffix: "lote".into(),
            ..Default::default()
        },
        &JobContext::silent(),
    )
    .unwrap();

    assert_eq!(report.outputs.len(), 2);
    assert!(
        report
            .outputs
            .iter()
            .all(|p| p.starts_with(dir.path().join("docs (lote)")))
    );
    let pages = |p: &std::path::Path| {
        use pliego_pdf::Document;
        Document::open(p.to_str().unwrap()).unwrap().page_count().unwrap()
    };
    assert_eq!(pages(&report.outputs[0]), 2);
    assert_eq!(pages(&report.outputs[1]), 1);
    assert_eq!(pages(&docs.join("a.pdf")), 3);
}
