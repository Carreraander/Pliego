mod common;

use common::*;
use pliego_formdesign::*;

fn spec(name: &str, kind: FieldType, y: f32) -> FieldSpec {
    FieldSpec {
        name: name.into(),
        kind,
        rect: [50.0, y, 250.0, y + 20.0],
        ..Default::default()
    }
}

#[test]
fn add_all_kinds_and_list() {
    let mut doc = open_blank();
    add_field(
        &mut doc,
        &FieldSpec {
            required: true,
            max_len: Some(30),
            tooltip: Some("Nombre completo".into()),
            ..spec("nombre", FieldType::Text, 700.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            value: Some("true".into()),
            ..spec("acepto", FieldType::CheckBox, 650.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            export_value: Some("Mujer".into()),
            value: Some("Mujer".into()),
            ..spec("sexo", FieldType::RadioButton, 600.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            export_value: Some("Hombre".into()),
            ..spec("sexo", FieldType::RadioButton, 570.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            options: vec!["Madrid".into(), "Vigo".into()],
            value: Some("Vigo".into()),
            ..spec("provincia", FieldType::ComboBox, 520.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            options: vec!["Español".into(), "Catalán".into()],
            page: 1,
            ..spec("idioma", FieldType::ListBox, 400.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            caption: Some("Enviar".into()),
            ..spec("enviar", FieldType::PushButton, 300.0)
        },
    )
    .unwrap();
    add_field(&mut doc, &spec("firma", FieldType::Signature, 200.0)).unwrap();

    let doc = reopen(&doc);
    let fs = list_fields(&doc).unwrap();
    let get = |n: &str| fs.iter().filter(|f| f.name == n).collect::<Vec<_>>();
    let nombre = get("nombre")[0];
    assert_eq!(nombre.kind, FieldType::Text);
    assert!(nombre.required);
    assert_eq!(nombre.max_len, Some(30));
    assert_eq!(nombre.tooltip.as_deref(), Some("Nombre completo"));
    assert_eq!(get("acepto")[0].value.as_deref(), Some("Yes"));
    let sexo = get("sexo");
    assert_eq!(sexo.len(), 2, "un botón por radio");
    assert_eq!(sexo[0].export_value.as_deref(), Some("Mujer"));
    assert_eq!(sexo[1].export_value.as_deref(), Some("Hombre"));
    assert_eq!(get("provincia")[0].options, ["Madrid", "Vigo"]);
    assert_eq!(get("provincia")[0].value.as_deref(), Some("Vigo"));
    assert_eq!(get("idioma")[0].page, 1);
    assert_eq!(get("idioma")[0].options, ["Español", "Catalán"]);
    assert_eq!(get("enviar")[0].kind, FieldType::PushButton);
    assert_eq!(get("firma")[0].kind, FieldType::Signature);
}

#[test]
fn rejects_bad_input() {
    let mut doc = open_blank();
    assert!(add_field(&mut doc, &spec("", FieldType::Text, 100.0)).is_err());
    assert!(add_field(&mut doc, &spec("a.b", FieldType::Text, 100.0)).is_err());
    assert!(
        add_field(
            &mut doc,
            &FieldSpec {
                rect: [0.0; 4],
                ..spec("x", FieldType::Text, 0.0)
            }
        )
        .is_err()
    );
    assert!(
        add_field(
            &mut doc,
            &FieldSpec {
                page: 5,
                ..spec("x", FieldType::Text, 100.0)
            }
        )
        .is_err()
    );
    assert!(
        add_field(&mut doc, &spec("r", FieldType::RadioButton, 100.0)).is_err(),
        "radio sin valor de exportación"
    );
    add_field(&mut doc, &spec("a", FieldType::Text, 100.0)).unwrap();
    assert!(
        add_field(&mut doc, &spec("a", FieldType::Text, 150.0)).is_err(),
        "nombre duplicado"
    );
    assert!(
        add_field(
            &mut doc,
            &FieldSpec {
                export_value: Some("x".into()),
                ..spec("a", FieldType::RadioButton, 150.0)
            }
        )
        .is_err(),
        "un radio no se une a un campo de texto"
    );
}

#[test]
fn text_field_has_appearance_and_value_survives() {
    let mut doc = open_blank();
    add_field(
        &mut doc,
        &FieldSpec {
            value: Some("Año 2026 ñ".into()),
            ..spec("t", FieldType::Text, 700.0)
        },
    )
    .unwrap();
    let doc = reopen(&doc);
    assert_eq!(list_fields(&doc).unwrap()[0].value.as_deref(), Some("Año 2026 ñ"));
    let page = doc.load_pdf_page(0).unwrap();
    let w = page.widgets().next().unwrap();
    assert!(
        w.annotation().object().get_dict("AP").unwrap().is_some(),
        "MuPDF genera el aspecto"
    );
}

#[test]
fn update_properties_and_rename() {
    let mut doc = open_blank();
    add_field(&mut doc, &spec("t", FieldType::Text, 700.0)).unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            options: vec!["a".into()],
            ..spec("c", FieldType::ComboBox, 650.0)
        },
    )
    .unwrap();
    update_field(
        &mut doc,
        "t",
        &FieldPatch {
            name: Some("texto".into()),
            required: Some(true),
            read_only: Some(true),
            multiline: Some(true),
            max_len: Some(10),
            tooltip: Some("ayuda".into()),
            value: Some("hola".into()),
            rect: Some([10.0, 10.0, 110.0, 40.0]),
            ..Default::default()
        },
    )
    .unwrap();
    update_field(
        &mut doc,
        "c",
        &FieldPatch {
            options: Some(vec!["x".into(), "y".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(update_field(&mut doc, "nada", &FieldPatch::default()).is_err());
    assert!(
        update_field(
            &mut doc,
            "texto",
            &FieldPatch {
                name: Some("c".into()),
                ..Default::default()
            }
        )
        .is_err()
    );

    let doc = reopen(&doc);
    let fs = list_fields(&doc).unwrap();
    let t = fs.iter().find(|f| f.name == "texto").unwrap();
    assert!(t.required && t.read_only && t.multiline);
    assert_eq!(t.max_len, Some(10));
    assert_eq!(t.value.as_deref(), Some("hola"));
    assert_eq!(t.rect, [10.0, 10.0, 110.0, 40.0]);
    assert_eq!(fs.iter().find(|f| f.name == "c").unwrap().options, ["x", "y"]);
    assert!(fs.iter().all(|f| f.name != "t"));
}

#[test]
fn update_checkbox_and_radio_value() {
    let mut doc = open_blank();
    add_field(&mut doc, &spec("ok", FieldType::CheckBox, 700.0)).unwrap();
    for (v, y) in [("A", 650.0), ("B", 620.0)] {
        add_field(
            &mut doc,
            &FieldSpec {
                export_value: Some(v.into()),
                ..spec("g", FieldType::RadioButton, y)
            },
        )
        .unwrap();
    }
    update_field(
        &mut doc,
        "ok",
        &FieldPatch {
            value: Some("true".into()),
            ..Default::default()
        },
    )
    .unwrap();
    update_field(
        &mut doc,
        "g",
        &FieldPatch {
            value: Some("B".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        update_field(
            &mut doc,
            "g",
            &FieldPatch {
                value: Some("Z".into()),
                ..Default::default()
            }
        )
        .is_err()
    );
    let doc = reopen(&doc);
    let fs = list_fields(&doc).unwrap();
    assert_eq!(
        fs.iter().find(|f| f.name == "ok").unwrap().value.as_deref(),
        Some("Yes")
    );
    assert!(
        fs.iter()
            .filter(|f| f.name == "g")
            .all(|f| f.value.as_deref() == Some("B"))
    );
    let mut doc = doc;
    update_field(
        &mut doc,
        "ok",
        &FieldPatch {
            value: Some(String::new()),
            ..Default::default()
        },
    )
    .unwrap();
    let doc = reopen(&doc);
    assert_ne!(
        list_fields(&doc)
            .unwrap()
            .iter()
            .find(|f| f.name == "ok")
            .unwrap()
            .value
            .as_deref(),
        Some("Yes")
    );
}

#[test]
fn delete_field_and_radio_group() {
    let mut doc = open_blank();
    add_field(&mut doc, &spec("t", FieldType::Text, 700.0)).unwrap();
    for (v, y) in [("A", 650.0), ("B", 620.0)] {
        add_field(
            &mut doc,
            &FieldSpec {
                export_value: Some(v.into()),
                ..spec("g", FieldType::RadioButton, y)
            },
        )
        .unwrap();
    }
    delete_field(&mut doc, "g").unwrap();
    assert!(delete_field(&mut doc, "g").is_err());
    let doc2 = reopen(&doc);
    let names: Vec<_> = list_fields(&doc2).unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["t"]);
    let fields = doc2
        .catalog()
        .unwrap()
        .get_dict("AcroForm")
        .unwrap()
        .unwrap()
        .get_dict("Fields")
        .unwrap()
        .unwrap();
    assert_eq!(fields.len().unwrap(), 1, "el grupo desaparece de /Fields");
    let mut doc = doc2;
    delete_field(&mut doc, "t").unwrap();
    assert!(list_fields(&reopen(&doc)).unwrap().is_empty());
}

#[test]
fn tab_order_reorders_fields() {
    let mut doc = open_blank();
    for (n, y) in [("a", 700.0), ("b", 650.0), ("c", 600.0)] {
        add_field(&mut doc, &spec(n, FieldType::Text, y)).unwrap();
    }
    assert_eq!(tab_order(&doc, 0).unwrap(), ["a", "b", "c"]);
    set_tab_order(&mut doc, 0, &["c".into(), "a".into(), "b".into()]).unwrap();
    assert_eq!(tab_order(&reopen(&doc), 0).unwrap(), ["c", "a", "b"]);
    assert!(set_tab_order(&mut doc, 0, &["z".into()]).is_err());
    assert!(set_tab_order(&mut doc, 0, &["a".into(), "a".into()]).is_err());
    assert!(set_tab_order(&mut doc, 9, &[]).is_err());
}

/// Cuenta los píxeles no blancos dentro del rectángulo (espacio PDF) de la página 0 renderizada a 72 ppp.
fn ink(doc: &mupdf::pdf::PdfDocument, rect: [f32; 4]) -> usize {
    use mupdf::{Colorspace, Matrix};
    let page = doc.load_pdf_page(0).unwrap();
    let pm = page
        .to_pixmap(&Matrix::IDENTITY, &Colorspace::device_rgb(), false, true)
        .unwrap();
    let (w, n) = (pm.width() as usize, pm.n() as usize);
    let h = pm.height() as f32;
    let s = pm.samples();
    let (x0, x1) = (rect[0] as usize, rect[2] as usize);
    let (y0, y1) = ((h - rect[3]) as usize, (h - rect[1]) as usize);
    let mut count = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            if s[(y * w + x) * n] < 200 {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn created_fields_render() {
    let mut doc = open_blank();
    let r = |y: f32| [50.0, y, 250.0, y + 20.0];
    add_field(
        &mut doc,
        &FieldSpec {
            value: Some("Hola".into()),
            ..spec("t", FieldType::Text, 700.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            value: Some("true".into()),
            rect: [50.0, 650.0, 70.0, 670.0],
            ..spec("c", FieldType::CheckBox, 650.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            caption: Some("Enviar".into()),
            ..spec("b", FieldType::PushButton, 600.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            options: vec!["a".into()],
            value: Some("a".into()),
            ..spec("o", FieldType::ComboBox, 550.0)
        },
    )
    .unwrap();
    let doc = reopen(&doc);
    // Un recuadro vacío tiene solo el borde (~160 px); con texto hay bastante más.
    assert!(ink(&doc, r(700.0)) > 200, "texto");
    assert!(ink(&doc, [50.0, 650.0, 70.0, 670.0]) > 60, "casilla marcada");
    assert!(ink(&doc, r(600.0)) > 200, "botón");
    assert!(ink(&doc, r(550.0)) > 200, "combo");
}
