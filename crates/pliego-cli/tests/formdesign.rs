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
