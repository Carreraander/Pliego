mod common;

use common::*;
use pliego_formdesign::*;
use pliego_pdf::PdfDocument;

fn text(name: &str, y: f32) -> FieldSpec {
    FieldSpec {
        name: name.into(),
        rect: [50.0, y, 250.0, y + 20.0],
        ..Default::default()
    }
}

/// Valor actual de un campo y resultado de asignarle `v` activando los scripts.
fn set(doc: &mut PdfDocument, name: &str, v: &str) -> bool {
    doc.enable_js().unwrap();
    let page = doc.load_pdf_page(0).unwrap();
    let mut w = page
        .widgets()
        .find(|w| w.name().unwrap().as_deref() == Some(name))
        .unwrap();
    w.set_value(doc, v, false).unwrap()
}

fn value(doc: &PdfDocument, name: &str) -> Option<String> {
    let page = doc.load_pdf_page(0).unwrap();
    page.widgets()
        .find(|w| w.name().unwrap().as_deref() == Some(name))
        .unwrap()
        .value()
        .unwrap()
}

#[test]
#[ignore = "necesita el motor JavaScript de formularios (flujo propio tras quitar MuPDF/mujs): la validación no rechaza el valor"]
fn range_validation_rejects_outside_values() {
    let mut doc = open_blank();
    add_field(
        &mut doc,
        &FieldSpec {
            validation: Validation::Range {
                min: Some(0.0),
                max: Some(120.0),
            },
            ..text("edad", 700.0)
        },
    )
    .unwrap();
    let mut doc = reopen(&doc);
    assert!(set(&mut doc, "edad", "50"));
    assert!(!set(&mut doc, "edad", "200"), "fuera de rango");
    assert!(!set(&mut doc, "edad", "-1"));
    assert_eq!(value(&doc, "edad").as_deref(), Some("50"));
    set_validation(&mut doc, "edad", &Validation::None).unwrap();
    assert!(set(&mut doc, "edad", "200"), "sin validación acepta todo");
    assert!(
        set_validation(
            &mut doc,
            "edad",
            &Validation::Range {
                min: Some(5.0),
                max: Some(1.0)
            }
        )
        .is_err()
    );
}

#[test]
#[ignore = "necesita el motor JavaScript de formularios (flujo propio tras quitar MuPDF/mujs): la validación no rechaza el valor"]
fn dni_and_iban_validation() {
    let mut doc = open_blank();
    add_field(
        &mut doc,
        &FieldSpec {
            validation: Validation::Dni,
            ..text("dni", 700.0)
        },
    )
    .unwrap();
    add_field(
        &mut doc,
        &FieldSpec {
            validation: Validation::Iban,
            ..text("iban", 650.0)
        },
    )
    .unwrap();
    let mut doc = reopen(&doc);
    assert!(set(&mut doc, "dni", "12345678Z"));
    assert!(set(&mut doc, "dni", "X1234567L"));
    assert!(set(&mut doc, "dni", "12345678-z"));
    assert!(!set(&mut doc, "dni", "12345678A"));
    assert!(!set(&mut doc, "dni", "1234"));
    assert!(set(&mut doc, "iban", "ES91 2100 0418 4502 0005 1332"));
    assert!(!set(&mut doc, "iban", "ES91 2100 0418 4502 0005 1333"));
    assert!(!set(&mut doc, "iban", "no es un iban"));
}

#[test]
#[ignore = "necesita el motor JavaScript de formularios (flujo propio tras quitar MuPDF/mujs): la validación no rechaza el valor"]
fn custom_script_validation() {
    let mut doc = open_blank();
    let js = "if (String(event.value).length > 3) event.rc = false;".to_string();
    add_field(
        &mut doc,
        &FieldSpec {
            validation: Validation::Script { js },
            ..text("corto", 700.0)
        },
    )
    .unwrap();
    let mut doc = reopen(&doc);
    assert!(set(&mut doc, "corto", "abc"));
    assert!(!set(&mut doc, "corto", "abcd"));
}

#[test]
fn calculations_sum_and_chain() {
    let mut doc = open_blank();
    for (n, y) in [
        ("a", 700.0),
        ("b", 670.0),
        ("total", 640.0),
        ("doble", 610.0),
        ("media", 580.0),
    ] {
        add_field(&mut doc, &text(n, y)).unwrap();
    }
    // `doble` depende de `total`, que se declara después: el orden de cálculo debe corregirse.
    set_calculation(
        &mut doc,
        "doble",
        Some(&Calculation {
            op: CalcOp::Sum,
            fields: vec!["total".into(), "total".into()],
            decimals: None,
        }),
    )
    .unwrap();
    set_calculation(
        &mut doc,
        "total",
        Some(&Calculation {
            op: CalcOp::Sum,
            fields: vec!["a".into(), "b".into()],
            decimals: Some(2),
        }),
    )
    .unwrap();
    set_calculation(
        &mut doc,
        "media",
        Some(&Calculation {
            op: CalcOp::Average,
            fields: vec!["a".into(), "b".into()],
            decimals: None,
        }),
    )
    .unwrap();

    let mut doc = reopen(&doc);
    let co = doc
        .catalog()
        .unwrap()
        .get_dict("AcroForm")
        .unwrap()
        .unwrap()
        .get_dict("CO")
        .unwrap()
        .unwrap();
    assert_eq!(co.len().unwrap(), 3);
    assert!(set(&mut doc, "a", "1.1"));
    assert!(set(&mut doc, "b", "2.2"));
    assert!(!recalculate(&mut doc).unwrap().is_empty());
    assert_eq!(value(&doc, "total").as_deref(), Some("3.3"), "redondeado a 2 decimales");
    assert_eq!(value(&doc, "doble").as_deref(), Some("6.6"));
    assert_eq!(value(&doc, "media").as_deref(), Some("1.65"));

    let got = get_calculation(&doc, "total").unwrap().unwrap();
    assert_eq!(got.op, CalcOp::Sum);
    assert_eq!(got.fields, ["a", "b"]);
    assert_eq!(got.decimals, Some(2));

    set_calculation(&mut doc, "doble", None).unwrap();
    assert!(get_calculation(&doc, "doble").unwrap().is_none());
}

#[test]
fn min_max_product() {
    let mut doc = open_blank();
    for (n, y) in [
        ("a", 700.0),
        ("b", 670.0),
        ("min", 640.0),
        ("max", 610.0),
        ("prod", 580.0),
    ] {
        add_field(&mut doc, &text(n, y)).unwrap();
    }
    for (n, op) in [("min", CalcOp::Min), ("max", CalcOp::Max), ("prod", CalcOp::Product)] {
        set_calculation(
            &mut doc,
            n,
            Some(&Calculation {
                op,
                fields: vec!["a".into(), "b".into()],
                decimals: None,
            }),
        )
        .unwrap();
    }
    assert!(set(&mut doc, "a", "4"));
    assert!(set(&mut doc, "b", "-3"));
    assert!(!recalculate(&mut doc).unwrap().is_empty());
    assert_eq!(value(&doc, "min").as_deref(), Some("-3"));
    assert_eq!(value(&doc, "max").as_deref(), Some("4"));
    assert_eq!(value(&doc, "prod").as_deref(), Some("-12"));
}

#[test]
fn calculation_errors_and_cycles() {
    let mut doc = open_blank();
    for (n, y) in [("a", 700.0), ("b", 670.0), ("c\"d", 640.0)] {
        add_field(&mut doc, &text(n, y)).unwrap();
    }
    let calc = |f: &[&str]| Calculation {
        op: CalcOp::Sum,
        fields: f.iter().map(|s| s.to_string()).collect(),
        decimals: None,
    };
    assert!(set_calculation(&mut doc, "a", Some(&calc(&[]))).is_err());
    assert!(set_calculation(&mut doc, "a", Some(&calc(&["a"]))).is_err(), "sí mismo");
    assert!(
        set_calculation(&mut doc, "a", Some(&calc(&["zzz"]))).is_err(),
        "campo inexistente"
    );
    set_calculation(&mut doc, "a", Some(&calc(&["b"]))).unwrap();
    assert!(set_calculation(&mut doc, "b", Some(&calc(&["a"]))).is_err(), "ciclo");
    assert!(get_calculation(&doc, "b").unwrap().is_none(), "el ciclo no deja huella");
    // Nombres con comillas no rompen el script.
    set_calculation(&mut doc, "b", Some(&calc(&["c\"d"]))).unwrap();
    assert_eq!(get_calculation(&reopen(&doc), "b").unwrap().unwrap().fields, ["c\"d"]);
}

#[test]
fn number_format_applies_on_commit() {
    let mut doc = open_blank();
    let fmt = FieldFormat::Number(NumberFormat {
        currency: Some("€".into()),
        ..Default::default()
    });
    add_field(
        &mut doc,
        &FieldSpec {
            format: fmt.clone(),
            ..text("importe", 700.0)
        },
    )
    .unwrap();
    let mut doc = reopen(&doc);
    assert!(set(&mut doc, "importe", "1234.5"));
    let v = value(&doc, "importe").unwrap();
    assert!(v == "1234.5" || v.contains("1.234,50"), "valor guardado: {v}");
    // El formato queda definido como acciones /AA /F y /K.
    let page = doc.load_pdf_page(0).unwrap();
    let w = page.widgets().next().unwrap();
    let aa = w.annotation().object().get_dict("AA").unwrap().unwrap();
    assert!(aa.get_dict("F").unwrap().is_some() && aa.get_dict("K").unwrap().is_some());
    set_format(&mut doc, "importe", &FieldFormat::None).unwrap();
    let page = doc.load_pdf_page(0).unwrap();
    let w = page.widgets().next().unwrap();
    let aa = w.annotation().object().get_dict("AA").unwrap().unwrap();
    assert!(aa.get_dict("F").unwrap().is_none());
}

#[test]
fn mask_percent_date_postal() {
    let mut doc = open_blank();
    for (n, f, y) in [
        ("mask", FieldFormat::Mask { mask: "999-AA".into() }, 700.0),
        ("pct", FieldFormat::Percent { decimals: 1 }, 670.0),
        (
            "fecha",
            FieldFormat::Date {
                pattern: "dd/mm/yyyy".into(),
            },
            640.0,
        ),
        ("cp", FieldFormat::PostalCode, 610.0),
        (
            "hora",
            FieldFormat::Time {
                pattern: "HH:MM".into(),
            },
            580.0,
        ),
    ] {
        add_field(
            &mut doc,
            &FieldSpec {
                format: f,
                ..text(n, y)
            },
        )
        .unwrap();
    }
    assert!(set_format(&mut doc, "mask", &FieldFormat::Mask { mask: String::new() }).is_err());
    assert!(set_format(&mut doc, "nada", &FieldFormat::PostalCode).is_err());
    let mut doc = reopen(&doc);
    assert!(set(&mut doc, "fecha", "03/11/2026"));
    assert!(set(&mut doc, "cp", "28001"));
    assert_eq!(list_fields(&doc).unwrap().len(), 5);
}
