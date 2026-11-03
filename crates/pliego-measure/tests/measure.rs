use std::path::PathBuf;

use mupdf::Document;
use mupdf::pdf::PdfDocument;
use pliego_measure::{Scale, Shape, Unit, add_measurement, list_measurements, page_scale, set_scale};

fn open(rel: &str) -> PdfDocument {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    let bytes = std::fs::read(p).unwrap();
    PdfDocument::try_from(Document::from_bytes(&bytes, "application/pdf").unwrap()).unwrap()
}

fn reopen(doc: &PdfDocument) -> PdfDocument {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("o.pdf");
    doc.save(out.to_str().unwrap()).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    PdfDocument::try_from(Document::from_bytes(&bytes, "application/pdf").unwrap()).unwrap()
}

#[test]
fn scale_and_measure_annotations_round_trip_through_save() {
    let mut doc = open("organize/a.pdf");
    assert_eq!(page_scale(&doc, 0).unwrap(), None);
    let scale = Scale::from_ratio(1.0, Unit::Cm, 5.0, Unit::M).unwrap();
    set_scale(&mut doc, 0, &scale).unwrap();

    let cm = Unit::Cm.points() as f32;
    let (m, _) = add_measurement(&mut doc, 0, &scale, &[[10.0, 10.0], [10.0 + 2.0 * cm, 10.0]], false).unwrap();
    assert_eq!(m.shape, Shape::Line);
    assert!((m.length - 10.0).abs() < 1e-3);
    let rect = [
        [50.0, 50.0],
        [50.0 + 2.0 * cm, 50.0],
        [50.0 + 2.0 * cm, 50.0 + cm],
        [50.0, 50.0 + cm],
    ];
    let (r, _) = add_measurement(&mut doc, 0, &scale, &rect, true).unwrap();
    assert!((r.area.unwrap() - 50.0).abs() < 0.05);

    let saved = reopen(&doc);
    let got = page_scale(&saved, 0).unwrap().unwrap();
    assert_eq!(got.unit, Unit::M);
    assert!((got.units_per_point - scale.units_per_point).abs() / scale.units_per_point < 1e-4);
    assert_eq!(page_scale(&saved, 1).unwrap(), None);

    let list = list_measurements(&saved, None).unwrap();
    assert_eq!(list.len(), 2, "{list:?}");
    let line = list.iter().find(|a| a.measurement.shape == Shape::Line).unwrap();
    assert!((line.measurement.length - 10.0).abs() < 0.01);
    let poly = list.iter().find(|a| a.measurement.shape == Shape::Polygon).unwrap();
    assert!((poly.measurement.area.unwrap() - 50.0).abs() < 0.1);
    assert!((poly.measurement.length - 30.0).abs() < 0.01);
}

#[test]
fn page_out_of_range_is_an_error() {
    let mut doc = open("organize/a.pdf");
    let s = Scale::calibrate(72.0, 1.0, Unit::M).unwrap();
    assert!(set_scale(&mut doc, 99, &s).is_err());
}
