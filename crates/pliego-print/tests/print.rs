use std::path::PathBuf;

use pliego_core::JobContext;
use pliego_print::{FindingCode, InkKind, PlateChannel, Profile, Severity, inks, open, preflight, separations};

fn doc(name: &str) -> pliego_print::PdfDocument {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/print")
        .join(name);
    open(&p, None).unwrap()
}

fn codes(name: &str, profile: &Profile) -> Vec<FindingCode> {
    let r = preflight(&doc(name), profile, &JobContext::silent()).unwrap();
    r.findings.iter().map(|f| f.code).collect()
}

#[test]
fn preflight_detects_every_defect() {
    let c = codes("defectos.pdf", &Profile::print());
    for expected in [
        FindingCode::FontNotEmbedded,
        FindingCode::LowResolution,
        FindingCode::HairLine,
        FindingCode::RgbContent,
        FindingCode::TooManySpotInks,
        FindingCode::MissingTrimBox,
    ] {
        assert!(c.contains(&expected), "falta {expected:?} en {c:?}");
    }
}

#[test]
fn preflight_report_details() {
    let r = preflight(&doc("defectos.pdf"), &Profile::print(), &JobContext::silent()).unwrap();
    assert!(!r.passed());
    let low = r
        .findings
        .iter()
        .find(|f| f.code == FindingCode::LowResolution)
        .unwrap();
    assert!(low.message.contains("5 ppp"), "{}", low.message);
    let font = r
        .findings
        .iter()
        .find(|f| f.code == FindingCode::FontNotEmbedded)
        .unwrap();
    assert!(font.message.contains("Helvetica"));
    assert_eq!(font.severity, Severity::Error);
    // Serializable para la app y la CLI.
    assert!(serde_json::to_string(&r).unwrap().contains("font_not_embedded"));
}

#[test]
fn clean_file_passes_every_profile() {
    for profile in [Profile::basic(), Profile::print()] {
        let r = preflight(&doc("limpio.pdf"), &profile, &JobContext::silent()).unwrap();
        assert!(r.findings.is_empty(), "{:?}", r.findings);
    }
    // PDF/X exige además la intención de salida, que el archivo de prueba no lleva.
    assert_eq!(
        codes("limpio.pdf", &Profile::pdfx()),
        [FindingCode::MissingOutputIntent]
    );
}

#[test]
fn lists_process_and_spot_inks() {
    let list = inks(&doc("defectos.pdf")).unwrap();
    let spots: Vec<_> = list
        .iter()
        .filter(|i| i.kind == InkKind::Spot)
        .map(|i| i.name.as_str())
        .collect();
    assert_eq!(spots, ["Barniz", "PANTONE 185 C", "PANTONE 300 C"]);
    assert!(list.iter().all(|i| i.pages == [0]));

    let process: Vec<_> = inks(&doc("limpio.pdf")).unwrap().into_iter().map(|i| i.name).collect();
    assert_eq!(process, ["Cyan", "Magenta", "Yellow", "Black"]);

    // Un documento solo RGB no tiene tintas de proceso.
    assert!(
        inks(&doc("defectos.pdf"))
            .unwrap()
            .iter()
            .all(|i| i.kind == InkKind::Spot)
    );
}

fn plate_coverage(name: &str) -> (Vec<f32>, f32) {
    let s = separations(&doc(name), 0, 36.0).unwrap();
    assert_eq!(s.plates.len(), 4);
    (s.plates.iter().map(|p| p.coverage()).collect(), s.max_total_ink)
}

#[test]
fn separations_match_known_cmyk_values() {
    let near = |a: f32, b: f32| (a - b).abs() < 0.02;
    for (file, expected) in [
        ("cmyk-cian.pdf", [1.0, 0.0, 0.0, 0.0]),
        ("cmyk-magenta.pdf", [0.0, 1.0, 0.0, 0.0]),
        ("cmyk-amarillo.pdf", [0.0, 0.0, 1.0, 0.0]),
        ("cmyk-negro.pdf", [0.0, 0.0, 0.0, 1.0]),
        ("cmyk-mezcla.pdf", [0.5, 0.25, 0.75, 0.1]),
    ] {
        let (cov, _) = plate_coverage(file);
        for (i, e) in expected.iter().enumerate() {
            assert!(
                near(cov[i], *e),
                "{file}: plancha {:?} = {} (esperado {e})",
                PlateChannel::ALL[i],
                cov[i]
            );
        }
    }
    let (_, tac) = plate_coverage("cmyk-mezcla.pdf");
    assert!((tac - 1.6).abs() < 0.03, "{tac}");
}

#[test]
fn separations_rejects_bad_input() {
    let d = doc("cmyk-cian.pdf");
    assert!(separations(&d, 5, 72.0).is_err());
    assert!(separations(&d, 0, 0.0).is_err());
}

fn roundtrip(d: &pliego_print::PdfDocument) -> (tempfile::TempDir, pliego_print::PdfDocument) {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("salida.pdf");
    pliego_print::save(d, &out).unwrap();
    let reopened = open(&out, None).unwrap();
    (dir, reopened)
}

#[test]
fn fix_hairlines_raises_thin_strokes() {
    let mut d = doc("defectos.pdf");
    let fix = pliego_print::fix_hairlines(&mut d, 0.25, &JobContext::silent()).unwrap();
    assert_eq!(fix.widths_changed, 1);
    assert_eq!(fix.pages_changed, 1);
    let (_dir, fixed) = roundtrip(&d);
    let c: Vec<_> = preflight(&fixed, &Profile::print(), &JobContext::silent())
        .unwrap()
        .findings
        .into_iter()
        .map(|f| f.code)
        .collect();
    assert!(!c.contains(&FindingCode::HairLine), "{c:?}");
    // El resto del contenido sigue ahí.
    assert!(c.contains(&FindingCode::FontNotEmbedded));
    // Idempotente.
    let mut again = fixed;
    assert_eq!(
        pliego_print::fix_hairlines(&mut again, 0.25, &JobContext::silent())
            .unwrap()
            .widths_changed,
        0
    );
    assert!(pliego_print::fix_hairlines(&mut again, 0.0, &JobContext::silent()).is_err());
}

fn box_of(d: &pliego_print::PdfDocument, key: &str) -> Vec<f32> {
    let page = d.find_page(0).unwrap();
    let b = page.get_dict(key).unwrap().unwrap_or_else(|| panic!("falta {key}"));
    (0..4)
        .map(|i| b.get_array(i).unwrap().unwrap().as_float().unwrap())
        .collect()
}

#[test]
fn printer_marks_extend_boxes_and_draw_in_all_plates() {
    let before = separations(&doc("limpio.pdf"), 0, 72.0).unwrap();
    let mut d = doc("limpio.pdf");
    let opts = pliego_print::MarkOptions::default();
    assert_eq!(
        pliego_print::add_printer_marks(&mut d, &opts, None, &JobContext::silent()).unwrap(),
        1
    );
    let (_dir, marked) = roundtrip(&d);

    // El corte original (10..190) se conserva; el sangrado crece 3 mm y la página deja sitio a las marcas.
    assert_eq!(box_of(&marked, "TrimBox"), [10.0, 10.0, 190.0, 190.0]);
    let bleed = box_of(&marked, "BleedBox");
    assert!((bleed[0] - (10.0 - opts.bleed)).abs() < 0.01);
    let media = box_of(&marked, "MediaBox");
    let slug = opts.bleed + opts.gap + opts.length;
    assert!((media[0] - (10.0 - slug).min(0.0)).abs() < 0.01, "{media:?}");
    assert!(media[2] > 200.0);

    // Las marcas usan el color de registro: aparecen en las cuatro planchas.
    let after = separations(&marked, 0, 72.0).unwrap();
    for (a, b) in after.plates.iter().zip(&before.plates) {
        let dark = |p: &pliego_print::Plate| p.samples.iter().filter(|v| **v > 8).count();
        assert!(dark(a) > dark(b), "las marcas deben pintar la plancha {:?}", a.channel);
    }
    // El contenido original sigue intacto.
    assert!(inks(&marked).unwrap().iter().any(|i| i.name == "Black"));
}

#[test]
fn printer_marks_reject_bad_input() {
    let mut d = doc("limpio.pdf");
    let ctx = JobContext::silent();
    assert!(pliego_print::add_printer_marks(&mut d, &Default::default(), Some(&[3]), &ctx).is_err());
    let bad = pliego_print::MarkOptions {
        bleed: -1.0,
        ..Default::default()
    };
    assert!(pliego_print::add_printer_marks(&mut d, &bad, None, &ctx).is_err());
}

/// Perfil CMYK de prueba: el que trae Ghostscript. No se sube al repositorio.
fn test_profile() -> Option<PathBuf> {
    let p = PathBuf::from("/usr/share/color/icc/ghostscript/default_cmyk.icc");
    p.is_file().then_some(p)
}

fn pdfx_options(icc: PathBuf, level: pliego_print::PdfxLevel) -> pliego_print::PdfxOptions {
    pliego_print::PdfxOptions {
        level,
        icc_profile: icc,
        condition_identifier: "PRUEBA".into(),
        condition: "Condición de prueba".into(),
        title: None,
        ghostscript: None,
    }
}

#[test]
fn pdfx_rejects_bad_profile_and_same_file() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("malo.icc");
    std::fs::write(&bad, vec![0u8; 200]).unwrap();
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/print/limpio.pdf");
    let ctx = JobContext::silent();
    let opts = pdfx_options(bad, pliego_print::PdfxLevel::X1a);
    let err = pliego_print::to_pdfx(&input, &dir.path().join("x.pdf"), &opts, &ctx).unwrap_err();
    assert!(err.to_string().contains("ICC"), "{err}");
    let err = pliego_print::to_pdfx(&input, &input, &opts, &ctx).unwrap_err();
    assert!(err.to_string().contains("mismo"), "{err}");
}

#[test]
fn pdfx_conversion_adds_output_intent_when_ghostscript_is_available() {
    let (Some(icc), Some(_)) = (test_profile(), pliego_print::find_ghostscript()) else {
        eprintln!("Ghostscript o el perfil de prueba no están disponibles; se omite");
        return;
    };
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/print/defectos.pdf");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("x1a.pdf");
    let report = pliego_print::to_pdfx(
        &input,
        &out,
        &pdfx_options(icc, pliego_print::PdfxLevel::X1a),
        &JobContext::silent(),
    )
    .unwrap();
    assert_eq!(report.outputs, std::slice::from_ref(&out));
    let converted = open(&out, None).unwrap();
    // Sin RGB ni fallos de intención de salida: el perfil PDF/X solo se queja de lo que sigue mal en el original.
    let pf = preflight(&converted, &Profile::pdfx(), &JobContext::silent()).unwrap();
    assert!(
        !pf.findings.iter().any(|f| f.code == FindingCode::MissingOutputIntent),
        "{:?}",
        pf.findings
    );
    assert!(
        !pf.findings.iter().any(|f| f.code == FindingCode::RgbContent),
        "{:?}",
        pf.findings
    );
    let raw = std::fs::read(&out).unwrap();
    assert!(raw.windows(13).any(|w| w == b"PDF/X-1a:2001"));
}

/// Lee un PAM CMYK (`pamcmyk32` de Ghostscript): devuelve ancho, alto y los cuatro canales por separado.
fn read_pam_cmyk(bytes: &[u8]) -> (usize, usize, Vec<Vec<u8>>) {
    let end = bytes.windows(7).position(|w| w == b"ENDHDR\n").expect("cabecera PAM") + 7;
    let head = std::str::from_utf8(&bytes[..end]).unwrap();
    let field = |k: &str| -> usize {
        head.lines()
            .find_map(|l| l.strip_prefix(k).map(|v| v.trim().parse().unwrap()))
            .unwrap()
    };
    let (w, h) = (field("WIDTH"), field("HEIGHT"));
    assert_eq!(field("DEPTH"), 4);
    let mut planes = vec![Vec::with_capacity(w * h); 4];
    for px in bytes[end..].chunks_exact(4).take(w * h) {
        for (c, plane) in planes.iter_mut().enumerate() {
            plane.push(px[c]);
        }
    }
    (w, h, planes)
}

/// Compara las planchas con las de Ghostscript (herramienta de referencia) en los 5 PDF CMYK de prueba.
/// Se omite si no hay Ghostscript instalado.
#[test]
fn separations_match_ghostscript_reference() {
    let Some(gs) = pliego_print::find_ghostscript() else {
        eprintln!("Ghostscript no disponible: comparación omitida");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "cmyk-cian.pdf",
        "cmyk-magenta.pdf",
        "cmyk-amarillo.pdf",
        "cmyk-negro.pdf",
        "cmyk-mezcla.pdf",
    ] {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/print")
            .join(file);
        let out = dir.path().join(format!("{file}.pam"));
        let status = std::process::Command::new(&gs)
            .args(["-q", "-dBATCH", "-dNOPAUSE", "-sDEVICE=pamcmyk32", "-r72"])
            .arg(format!("-sOutputFile={}", out.display()))
            .arg(&src)
            .status()
            .unwrap();
        assert!(status.success(), "{file}: Ghostscript falló");
        let (w, h, reference) = read_pam_cmyk(&std::fs::read(&out).unwrap());

        let s = separations(&doc(file), 0, 72.0).unwrap();
        assert_eq!(
            (s.plates[0].width as usize, s.plates[0].height as usize),
            (w, h),
            "{file}"
        );
        for (plate, refp) in s.plates.iter().zip(&reference) {
            let total: u64 = plate
                .samples
                .iter()
                .zip(refp)
                .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64)
                .sum();
            let mean = total as f64 / (w * h) as f64;
            assert!(
                mean < 3.0,
                "{file}: plancha {:?} difiere de Ghostscript (diferencia media {mean:.2}/255)",
                plate.channel
            );
        }
    }
}
