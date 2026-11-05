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

/// PDF con dos campos numéricos (`a` entre 0 y 10, `b` libre) y un total calculado `t = a + b`.
fn scripted_pdf() -> Vec<u8> {
    let js = |c: &str| format!("<< /S /JavaScript /JS ({c}) >>");
    let objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R 6 0 R 8 0 R 9 0 R] /CO [6 0 R] \
         /DA (/Helv 0 Tf 0 g) /DR << /Font << /Helv 7 0 R >> >> >> >>"
            .into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /Helv 7 0 R >> >> \
         /Annots [4 0 R 5 0 R 6 0 R 8 0 R 9 0 R] >>"
            .into(),
        format!(
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (a) /Rect [50 700 250 720] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) \
             /AA << /V {} >> >>",
            js("AFRange_Validate(true, 0, true, 10);")
        ),
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (b) /Rect [50 660 250 680] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) >>"
            .into(),
        format!(
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (t) /Rect [50 620 250 640] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) \
             /AA << /C {} >> >>",
            js("AFSimple_Calculate('SUM', new Array('a','b'));")
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
        format!(
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (n) /Rect [50 580 250 600] /F 4 /P 3 0 R /DA (/Helv 12 Tf 0 g) \
             /AA << /K {} /F {} >> >>",
            js("AFNumber_Keystroke(2, 0, 0, 0, '', false);"),
            js("AFNumber_Format(2, 2, 0, 0, ' EUR', false);")
        ),
        format!(
            "<< /Type /Annot /Subtype /Widget /FT /Btn /Ff 65536 /T (limpiar) /Rect [300 700 400 720] /F 4 /P 3 0 R \
             /A {} >>",
            js("this.getField('b').value = ''; app.alert('Hecho');")
        ),
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

#[test]
fn fields_with_scripts_report_formatted_text_and_actions() {
    let mut doc = pliego_pdf::PdfDocument::from_bytes(&scripted_pdf()).unwrap();
    let mut v = FieldValues::new();
    v.insert("n".into(), FieldValue::Text("1234.5".into()));
    fill_with_scripts(&mut doc, &v).unwrap();
    let fs = fields_with_scripts(&mut doc).unwrap();
    let n = get(&fs, "n");
    assert_eq!(n.value.as_deref(), Some("1234.5"), "el valor es lo tecleado");
    assert_eq!(
        n.display.as_deref(),
        Some("1.234,50 EUR"),
        "y el texto se enseña con formato"
    );
    assert_eq!(n.scripts, ["F", "K"]);
    assert!(get(&fs, "limpiar").scripts.iter().any(|k| k == "A" || k == "U"));
    // Sin scripts no hay texto formateado.
    let fs = fields(&mut doc).unwrap();
    assert_eq!(get(&fs, "n").display, None);
    assert!(get(&fs, "n").scripts.is_empty());
}

#[test]
fn keystroke_filters_typing_and_buttons_run_actions() {
    let mut doc = pliego_pdf::PdfDocument::from_bytes(&scripted_pdf()).unwrap();
    let ok = keystroke(&mut doc, "n", "7", "12", (2, 2)).unwrap();
    assert!(ok.accepted && ok.change == "7", "{ok:?}");
    let bad = keystroke(&mut doc, "n", "x", "12", (2, 2)).unwrap();
    assert!(!bad.accepted, "una letra no entra en un campo numérico: {bad:?}");
    // Un campo sin script de tecleo, o que no existe, lo acepta todo.
    assert!(keystroke(&mut doc, "b", "x", "", (0, 0)).unwrap().accepted);
    assert!(keystroke(&mut doc, "nadie", "x", "", (0, 0)).unwrap().accepted);

    let mut v = FieldValues::new();
    v.insert("b".into(), FieldValue::Text("5".into()));
    fill_with_scripts(&mut doc, &v).unwrap();
    let msgs = run_action(&mut doc, "limpiar", "U")
        .unwrap()
        .expect("el botón tiene acción");
    assert_eq!(msgs, ["Hecho"]);
    let fs = fields(&mut doc).unwrap();
    assert_eq!(get(&fs, "b").value.as_deref(), Some(""), "el botón vació el campo b");
    assert_eq!(
        run_action(&mut doc, "n", "U").unwrap(),
        None,
        "el campo n no tiene acción de ratón"
    );
    assert_eq!(run_action(&mut doc, "nadie", "U").unwrap(), None);
}

#[test]
fn fill_with_scripts_validates_and_calculates() {
    let mut doc = pliego_pdf::PdfDocument::from_bytes(&scripted_pdf()).unwrap();
    let mut v = FieldValues::new();
    v.insert("a".into(), FieldValue::Text("4".into()));
    v.insert("b".into(), FieldValue::Text("2.5".into()));
    let r = fill_with_scripts(&mut doc, &v).unwrap();
    assert_eq!(r.filled.len(), 2, "{r:?}");
    assert!(r.rejected.is_empty(), "{r:?}");
    let fs = fields(&mut doc).unwrap();
    assert_eq!(get(&fs, "t").value.as_deref(), Some("6.5"), "el total se calcula");

    // Fuera de rango: la validación lo rechaza, avisa y el valor no cambia.
    let mut v = FieldValues::new();
    v.insert("a".into(), FieldValue::Text("99".into()));
    let r = fill_with_scripts(&mut doc, &v).unwrap();
    assert_eq!(r.rejected, ["a"], "{r:?}");
    assert!(!r.messages.is_empty(), "el aviso de la validación llega: {r:?}");
    let fs = fields(&mut doc).unwrap();
    assert_eq!(get(&fs, "a").value.as_deref(), Some("4"));

    // `fill` a secas no ejecuta scripts: acepta el valor y no recalcula.
    let mut doc = pliego_pdf::PdfDocument::from_bytes(&scripted_pdf()).unwrap();
    let r = fill(&mut doc, &v).unwrap();
    assert_eq!(r.filled, ["a"]);
    assert!(r.messages.is_empty());
}
