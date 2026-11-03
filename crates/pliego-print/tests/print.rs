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
