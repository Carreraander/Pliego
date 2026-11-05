mod common;

use common::*;
use pliego_forms::*;

fn get<'a>(fs: &'a [FieldInfo], n: &str) -> &'a FieldInfo {
    fs.iter()
        .find(|f| f.name == n)
        .unwrap_or_else(|| panic!("falta {n}: {fs:?}"))
}

#[test]
fn lists_all_field_kinds() {
    let mut doc = open_form();
    let fs = fields(&mut doc).unwrap();
    assert_eq!(fs.len(), 5, "{fs:?}");
    assert_eq!(get(&fs, "nombre").kind, FieldKind::Text);
    assert_eq!(get(&fs, "acepto").kind, FieldKind::CheckBox);
    assert_eq!(get(&fs, "sexo").kind, FieldKind::RadioButton);
    assert_eq!(get(&fs, "provincia").kind, FieldKind::ComboBox);
    assert_eq!(get(&fs, "idioma").kind, FieldKind::ListBox);
    assert_eq!(get(&fs, "provincia").options, ["Madrid", "Sevilla", "Vigo"]);
    let mut radio = get(&fs, "sexo").options.clone();
    radio.sort();
    assert_eq!(radio, ["Hombre", "Mujer"]);
}

#[test]
fn fill_and_reread_after_save() {
    let mut doc = open_form();
    let mut v = FieldValues::new();
    v.insert("nombre".into(), FieldValue::Text("Ñandú Pérez".into()));
    v.insert("acepto".into(), FieldValue::Bool(true));
    v.insert("sexo".into(), FieldValue::Text("Mujer".into()));
    v.insert("provincia".into(), FieldValue::Text("Vigo".into()));
    v.insert("idioma".into(), FieldValue::Text("Gallego".into()));
    v.insert("noexiste".into(), FieldValue::Text("x".into()));
    let r = fill(&mut doc, &v).unwrap();
    assert_eq!(r.not_found, ["noexiste"]);
    let mut doc = reopen(&doc);
    let fs = fields(&mut doc).unwrap();
    assert_eq!(get(&fs, "nombre").value.as_deref(), Some("Ñandú Pérez"));
    assert_eq!(get(&fs, "acepto").value.as_deref(), Some("Yes"));
    assert_eq!(get(&fs, "sexo").value.as_deref(), Some("Mujer"));
    assert_eq!(get(&fs, "provincia").value.as_deref(), Some("Vigo"));
    assert_eq!(get(&fs, "idioma").value.as_deref(), Some("Gallego"));
}

#[test]
fn reset_clears_values() {
    let mut doc = open_form();
    let mut v = FieldValues::new();
    v.insert("nombre".into(), FieldValue::Text("Ana".into()));
    fill(&mut doc, &v).unwrap();
    assert_eq!(reset(&mut doc, None).unwrap(), 6);
    let fs = fields(&mut doc).unwrap();
    assert!(get(&fs, "nombre").value.as_deref().unwrap_or("").is_empty());
}

#[test]
fn flatten_removes_fields_and_keeps_text() {
    let mut doc = open_form();
    let mut v = FieldValues::new();
    v.insert("nombre".into(), FieldValue::Text("Ana".into()));
    fill(&mut doc, &v).unwrap();
    assert_eq!(flatten(&mut doc).unwrap(), 5);
    let mut doc = reopen(&doc);
    assert!(fields(&mut doc).unwrap().is_empty());
}

#[test]
fn json_and_csv_export_import() {
    let mut a = open_form();
    let mut v = FieldValues::new();
    v.insert("nombre".into(), FieldValue::Text("Luis, el \"grande\"".into()));
    v.insert("acepto".into(), FieldValue::Bool(true));
    fill(&mut a, &v).unwrap();
    for fmt in [Format::Json, Format::Csv, Format::Fdf, Format::Xfdf] {
        let text = export_data(&mut a, fmt).unwrap();
        let mut b = open_form();
        import_data_bytes(&mut b, text.as_bytes(), fmt).unwrap();
        let fs = fields(&mut b).unwrap();
        assert_eq!(
            get(&fs, "nombre").value.as_deref(),
            Some("Luis, el \"grande\""),
            "{fmt:?}"
        );
        assert_eq!(get(&fs, "acepto").value.as_deref(), Some("Yes"), "{fmt:?}");
    }
}

#[test]
fn bulk_fill_makes_one_pdf_per_row_with_progress_and_cancel() {
    use pliego_core::{CancelToken, JobContext, Progress};
    use std::sync::Mutex;
    struct Rec(Mutex<Vec<u64>>);
    impl Progress for Rec {
        fn report(&self, done: u64, _t: u64, _m: &str) {
            self.0.lock().unwrap().push(done);
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let tpl = dir.path().join("plantilla.pdf");
    std::fs::write(&tpl, form_pdf()).unwrap();
    let csv = dir.path().join("datos.csv");
    std::fs::write(
        &csv,
        "nombre,acepto,provincia,_filename\nAna,sí,Madrid,ana\nLuis,no,Vigo,\nEva,1,Sevilla,\n",
    )
    .unwrap();
    let out = dir.path().join("salida");
    let rec = Rec(Mutex::new(vec![]));
    let ctx = JobContext::new(&rec, CancelToken::new());
    let rep = bulk_fill(&tpl, &csv, &out, false, &ctx).unwrap();
    assert_eq!(rep.outputs.len(), 3);
    assert!(out.join("ana.pdf").exists() && out.join("plantilla-002.pdf").exists());
    assert_eq!(*rec.0.lock().unwrap(), [0, 1, 2, 3]);
    let mut d = pliego_pdf::PdfDocument::open(out.join("plantilla-003.pdf")).unwrap();
    assert_eq!(get(&fields(&mut d).unwrap(), "nombre").value.as_deref(), Some("Eva"));

    // Cancelación: no genera nada más.
    let cancel = CancelToken::new();
    cancel.cancel();
    let out2 = dir.path().join("cancelado");
    let ctx = JobContext::new(&rec, cancel);
    assert!(matches!(
        bulk_fill(&tpl, &csv, &out2, false, &ctx),
        Err(pliego_core::Error::Cancelled)
    ));
}

#[test]
fn every_field_reports_a_rect_inside_its_page() {
    let mut doc = open_form();
    let fs = fields(&mut doc).unwrap();
    for f in &fs {
        let r = f.rect.unwrap_or_else(|| panic!("{} sin recuadro", f.name));
        assert!(r[2] > r[0] && r[3] > r[1], "{}: recuadro vacío {r:?}", f.name);
        assert!(r[0] >= 0.0 && r[1] >= 0.0, "{}: fuera de la página {r:?}", f.name);
    }
}
