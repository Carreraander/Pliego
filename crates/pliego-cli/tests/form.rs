use std::process::Command;

fn pliego() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pliego"));
    c.env("PLIEGO_LANG", "es");
    c
}

/// PDF con un campo de texto "nombre" y una casilla "acepto" (generado, sin datos reales).
fn form_pdf() -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R] /DA (/Helv 0 Tf 0 g) /DR << /Font << /Helv 6 0 R >> >> /NeedAppearances true >> >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Annots [4 0 R 5 0 R] >>",
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (nombre) /Rect [20 200 200 220] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) >>",
        "<< /Type /Annot /Subtype /Widget /FT /Btn /T (acepto) /Rect [20 150 40 170] /F 4 /P 3 0 R /V /Off /AS /Off >>",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
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

#[test]
fn fields_fill_export_bulk() {
    let dir = tempfile::tempdir().unwrap();
    let tpl = dir.path().join("f.pdf");
    std::fs::write(&tpl, form_pdf()).unwrap();

    let r = pliego().args(["--json", "form", "fields"]).arg(&tpl).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);

    let out = dir.path().join("lleno.pdf");
    let r = pliego()
        .args(["form", "fill"])
        .arg(&tpl)
        .args(["--set", "nombre=Ana", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));

    let r = pliego().args(["form", "export"]).arg(&out).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(v["nombre"], "Ana");

    for ext in ["xfdf", "fdf"] {
        let data = dir.path().join(format!("d.{ext}"));
        let r = pliego()
            .args(["form", "export"])
            .arg(&out)
            .args(["-f", ext, "-o"])
            .arg(&data)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let back = dir.path().join(format!("back-{ext}.pdf"));
        let r = pliego()
            .args(["form", "import"])
            .arg(&tpl)
            .arg(&data)
            .arg("-o")
            .arg(&back)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let r = pliego().args(["form", "export"]).arg(&back).output().unwrap();
        let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
        assert_eq!(v["nombre"], "Ana", "{ext}");
    }

    let csv = dir.path().join("d.csv");
    std::fs::write(&csv, "nombre\nA\nB\n").unwrap();
    let od = dir.path().join("lote");
    let r = pliego()
        .args(["form", "bulk"])
        .arg(&tpl)
        .arg(&csv)
        .arg("-o")
        .arg(&od)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(od.join("f-001.pdf").exists() && od.join("f-002.pdf").exists());

    // Sin -o no se sobrescribe la entrada.
    let r = pliego()
        .args(["form", "fill"])
        .arg(&tpl)
        .args(["--set", "nombre=X"])
        .output()
        .unwrap();
    assert!(!r.status.success());
}
