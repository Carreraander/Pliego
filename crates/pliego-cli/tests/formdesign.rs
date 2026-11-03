use std::process::Command;

fn pliego() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pliego"))
}

/// PDF de una página en blanco (generado, sin datos reales).
fn blank_pdf() -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
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

fn run(args: &[&str]) -> std::process::Output {
    let out = pliego().args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[test]
fn add_update_order_delete() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    std::fs::write(&a, blank_pdf()).unwrap();
    let p = |n: &str| dir.path().join(n).to_str().unwrap().to_string();
    let (a_s, b, c, d, e) = (a.to_str().unwrap(), p("b.pdf"), p("c.pdf"), p("d.pdf"), p("e.pdf"));

    run(&[
        "formdesign",
        "add",
        a_s,
        "-o",
        &b,
        "--name",
        "nombre",
        "--type",
        "text",
        "--rect",
        "50,700,250,720",
        "--required",
    ]);
    run(&[
        "formdesign",
        "add",
        &b,
        "-o",
        &c,
        "--name",
        "pais",
        "--type",
        "combo",
        "--rect",
        "50,650,250,670",
        "--option",
        "España",
        "--option",
        "Chile",
    ]);
    run(&[
        "formdesign",
        "update",
        &c,
        "-o",
        &d,
        "--field",
        "nombre",
        "--rename",
        "apellidos",
        "--tooltip",
        "Sus apellidos",
    ]);
    run(&["formdesign", "tab-order", &d, "-o", &e, "--order", "pais,apellidos"]);

    let out = run(&["--json", "formdesign", "list", &e]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let names: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["pais", "apellidos"]);
    assert_eq!(v[1]["tooltip"], "Sus apellidos");
    assert_eq!(v[0]["options"], serde_json::json!(["España", "Chile"]));

    run(&["formdesign", "delete", &e, "--in-place", "--field", "pais"]);
    let out = run(&["--json", "formdesign", "list", &e]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);

    // Sin -o ni --in-place no escribe nada.
    let bad = pliego()
        .args(["formdesign", "delete", &e, "--field", "apellidos"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    // La salida de `pliego form fields` (W03) ve los campos creados.
    let out = run(&["--json", "form", "fields", &e]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("apellidos"));
}

#[test]
fn calc_validate_format_recalc() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    std::fs::write(&a, blank_pdf()).unwrap();
    let p = |n: &str| dir.path().join(n).to_str().unwrap().to_string();
    let a_s = a.to_str().unwrap().to_string();
    let mut cur = a_s;
    let mut step = |args: &[&str]| {
        let next = p(&format!("s{}.pdf", std::time::Instant::now().elapsed().subsec_nanos()));
        let mut v: Vec<&str> = vec!["formdesign", args[0], &cur];
        v.extend(["-o", &next]);
        v.extend(&args[1..]);
        run(&v);
        cur = next;
    };
    step(&["add", "--name", "a", "--type", "text", "--rect", "50,700,150,720"]);
    step(&["add", "--name", "b", "--type", "text", "--rect", "50,670,150,690"]);
    step(&["add", "--name", "total", "--type", "text", "--rect", "50,640,150,660"]);
    step(&[
        "calc",
        "--field",
        "total",
        "--op",
        "sum",
        "--inputs",
        "a,b",
        "--decimals",
        "2",
    ]);
    step(&[
        "validate", "--field", "a", "--kind", "range", "--min", "0", "--max", "100",
    ]);
    step(&["format", "--field", "total", "--kind", "number", "--currency", "€"]);
    step(&["update", "--field", "a", "--value", "1.5"]);
    step(&["update", "--field", "b", "--value", "2.25"]);
    step(&["recalc"]);
    let out = run(&["--json", "form", "fields", &cur]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let total = v.as_array().unwrap().iter().find(|f| f["name"] == "total").unwrap();
    assert_eq!(total["value"], "3.75");
    // Fórmula inválida: ciclo.
    let bad = pliego()
        .args([
            "formdesign",
            "calc",
            &cur,
            "--in-place",
            "--field",
            "a",
            "--op",
            "sum",
            "--inputs",
            "total",
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}

#[test]
fn detect_creates_fields_in_flat_pdf() {
    // Formulario plano: tres subrayados con su etiqueta.
    let mut ops = String::new();
    for (i, l) in ["Nombre", "Apellidos", "Ciudad"].iter().enumerate() {
        let y = 700 - i * 40;
        ops.push_str(&format!(
            "BT /F1 10 Tf 50 {} Td ({l}:) Tj ET 0.8 w 130 {y} m 400 {y} l S\n",
            y + 2
        ));
    }
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{ops}\nendstream", ops.len()),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offs = vec![];
    for (i, o) in objs.iter().enumerate() {
        offs.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).into_bytes());
    }
    let x = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).into_bytes());
    for o in offs {
        pdf.extend(format!("{o:010} 00000 n \n").into_bytes());
    }
    pdf.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objs.len() + 1
        )
        .into_bytes(),
    );

    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("plano.pdf");
    std::fs::write(&a, pdf).unwrap();
    let out = dir.path().join("con-campos.pdf");
    let (a_s, o_s) = (a.to_str().unwrap(), out.to_str().unwrap());

    let v: serde_json::Value = serde_json::from_slice(&run(&["--json", "formdesign", "detect", a_s]).stdout).unwrap();
    let names: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["Nombre", "Apellidos", "Ciudad"]);
    assert!(!out.exists(), "sin -o no escribe nada");

    run(&["formdesign", "detect", a_s, "-o", o_s, "--pages", "1"]);
    let v: serde_json::Value = serde_json::from_slice(&run(&["--json", "formdesign", "list", o_s]).stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 3);
}
