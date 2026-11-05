//! Dibujo de campos de formulario con PDFium (validación del 2026-11-05, tras quitar MuPDF).
//!
//! Se crea un formulario con todos los tipos de campo, se rellena activando los scripts (formato, validación y
//! cálculo) y se dibuja por la misma ruta que usa el visor (`to_pixmap` con `show_extras`, que activa `FFLDraw`).
//! Se comprueba que cada campo deja tinta donde debe y que el texto formateado (`1.234,50 €`, `25 %`) no es el valor
//! crudo. Con `PLIEGO_FORMS_OUT=<carpeta>` se escriben el PDF y los PNG para mirarlos y compararlos con otros
//! motores (Poppler, Reader).
mod common;

use common::*;
use pliego_formdesign::*;
use pliego_pdf::{Colorspace, Matrix, PdfDocument};

fn field(name: &str, kind: FieldType, rect: [f32; 4]) -> FieldSpec {
    FieldSpec {
        name: name.into(),
        kind,
        rect,
        ..Default::default()
    }
}

fn set(doc: &mut PdfDocument, name: &str, v: &str) -> bool {
    let page = doc.load_pdf_page(0).unwrap();
    let mut w = page
        .widgets()
        .find(|w| w.name().unwrap().as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no hay campo {name}"));
    w.set_value(doc, v, false).unwrap()
}

/// Píxeles oscuros (<200) del rectángulo (espacio PDF) en un dibujo a `scale`.
fn ink(pm: &pliego_pdf::Pixmap, scale: f32, rect: [f32; 4]) -> usize {
    let (w, n, h) = (pm.width() as usize, pm.n() as usize, pm.height() as f32);
    let s = pm.samples();
    let (x0, x1) = ((rect[0] * scale) as usize, (rect[2] * scale) as usize);
    let (y0, y1) = ((h - rect[3] * scale) as usize, (h - rect[1] * scale) as usize);
    let mut c = 0;
    for y in y0..y1.min(pm.height() as usize) {
        for x in x0..x1.min(w) {
            if s[(y * w + x) * n] < 200 {
                c += 1;
            }
        }
    }
    c
}

fn render(doc: &PdfDocument, scale: f32) -> pliego_pdf::Pixmap {
    let page = doc.load_pdf_page(0).unwrap();
    page.to_pixmap(&Matrix::new_scale(scale, scale), Colorspace::Rgb, false, true)
        .unwrap()
}

#[test]
fn every_field_type_is_drawn_with_its_dynamic_text() {
    let mut doc = open_blank();
    let wide = |y: f32| [50.0, y, 300.0, y + 22.0];
    let mut add = |s: FieldSpec| add_field(&mut doc, &s).unwrap();
    add(field("nombre", FieldType::Text, wide(780.0)));
    add(FieldSpec {
        format: FieldFormat::Number(NumberFormat {
            decimals: 0,
            ..Default::default()
        }),
        ..field("cantidad", FieldType::Text, wide(745.0))
    });
    add(FieldSpec {
        format: FieldFormat::Number(NumberFormat {
            currency: Some("€".into()),
            ..Default::default()
        }),
        ..field("precio", FieldType::Text, wide(710.0))
    });
    add(FieldSpec {
        format: FieldFormat::Number(NumberFormat {
            currency: Some("€".into()),
            ..Default::default()
        }),
        read_only: true,
        ..field("total", FieldType::Text, wide(675.0))
    });
    add(FieldSpec {
        format: FieldFormat::Date {
            pattern: "dd/mm/yyyy".into(),
        },
        ..field("fecha", FieldType::Text, wide(640.0))
    });
    add(FieldSpec {
        format: FieldFormat::Percent { decimals: 1 },
        ..field("porcentaje", FieldType::Text, wide(605.0))
    });
    add(FieldSpec {
        multiline: true,
        ..field("notas", FieldType::Text, [50.0, 520.0, 300.0, 590.0])
    });
    add(FieldSpec {
        password: true,
        ..field("clave", FieldType::Text, wide(485.0))
    });
    add(field("acepto", FieldType::CheckBox, [50.0, 450.0, 70.0, 470.0]));
    add(field("nolo", FieldType::CheckBox, [90.0, 450.0, 110.0, 470.0]));
    for (i, v) in ["uno", "dos"].iter().enumerate() {
        add(FieldSpec {
            export_value: Some((*v).into()),
            ..field(
                "opcion",
                FieldType::RadioButton,
                [50.0 + 40.0 * i as f32, 415.0, 70.0 + 40.0 * i as f32, 435.0],
            )
        });
    }
    add(FieldSpec {
        options: vec!["Madrid".into(), "Sevilla".into(), "Bilbao".into()],
        ..field("ciudad", FieldType::ComboBox, wide(380.0))
    });
    add(FieldSpec {
        options: vec!["Rojo".into(), "Verde".into(), "Azul".into(), "Negro".into()],
        ..field("color", FieldType::ListBox, [50.0, 290.0, 200.0, 360.0])
    });
    add(FieldSpec {
        caption: Some("Enviar".into()),
        ..field("enviar", FieldType::PushButton, [50.0, 240.0, 150.0, 270.0])
    });
    add(field("firma", FieldType::Signature, [50.0, 170.0, 250.0, 220.0]));
    set_calculation(
        &mut doc,
        "total",
        Some(&Calculation {
            op: CalcOp::Product,
            fields: vec!["cantidad".into(), "precio".into()],
            decimals: Some(2),
        }),
    )
    .unwrap();

    let mut doc = reopen(&doc);
    doc.enable_js().unwrap();
    assert!(set(&mut doc, "nombre", "Ander Carrera"));
    assert!(set(&mut doc, "cantidad", "3"));
    assert!(set(&mut doc, "precio", "1234.5"));
    assert!(set(&mut doc, "fecha", "05/11/2026"));
    assert!(set(&mut doc, "porcentaje", "0.255"));
    assert!(set(&mut doc, "notas", "Primera línea\nSegunda línea\nTercera"));
    assert!(set(&mut doc, "clave", "secreto"));
    assert!(set(&mut doc, "acepto", "Yes"));
    assert!(set(&mut doc, "opcion", "dos"));
    assert!(set(&mut doc, "ciudad", "Sevilla"));
    assert!(set(&mut doc, "color", "Verde"));

    let messages = doc.take_js_messages();
    assert!(
        messages.is_empty(),
        "los scripts no deben dar avisos ni errores: {messages:?}"
    );
    // El valor guardado es el crudo; el cálculo lo hizo el motor.
    let page = doc.load_pdf_page(0).unwrap();
    let val = |n: &str| {
        page.widgets()
            .find(|w| w.name().unwrap().as_deref() == Some(n))
            .unwrap()
            .value()
            .unwrap()
    };
    assert_eq!(val("precio").as_deref(), Some("1234.5"));
    assert_eq!(val("total").as_deref(), Some("3703.5"), "cantidad × precio");

    // El texto formateado va en la apariencia y los visores no deben regenerarla desde el valor crudo.
    let mut bytes = Vec::new();
    doc.write_to(&mut bytes).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("/NeedAppearances false"),
        "con texto formateado se baja NeedAppearances"
    );
    assert!(
        text.contains("(1.234,50\\200)") && text.contains("(25,5%)"),
        "la apariencia lleva el texto formateado"
    );
    let saved = reopen(&doc);
    let out = std::env::var_os("PLIEGO_FORMS_OUT").map(std::path::PathBuf::from);
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir).unwrap();
        let mut buf = Vec::new();
        doc.write_to(&mut buf).unwrap();
        std::fs::write(dir.join("formulario-lleno.pdf"), buf).unwrap();
    }
    // Dibujo en vivo (scripts activos: el texto formateado se dibuja sin tocar el PDF) y dibujo del archivo guardado.
    for (label, d) in [("vivo", &doc), ("guardado", &saved)] {
        let pm = render(d, 2.0);
        if let Some(dir) = &out {
            std::fs::write(dir.join(format!("campos-{label}-pliego.png")), pm.encode_png().unwrap()).unwrap();
        }
        let has = |name: &str, rect: [f32; 4], min: usize| {
            let n = ink(&pm, 2.0, rect);
            assert!(n >= min, "{label}: «{name}» casi sin tinta ({n} < {min})");
        };
        has("nombre", wide(780.0), 400);
        has("cantidad", wide(745.0), 150);
        has("precio", wide(710.0), 400);
        has("total", wide(675.0), 400);
        has("fecha", wide(640.0), 400);
        has("porcentaje", wide(605.0), 250);
        has("notas", [50.0, 520.0, 300.0, 590.0], 1200);
        has("clave", wide(485.0), 300);
        has("acepto", [50.0, 450.0, 70.0, 470.0], 150);
        has("opcion", [90.0, 415.0, 110.0, 435.0], 150);
        has("ciudad", wide(380.0), 400);
        has("color", [50.0, 290.0, 200.0, 360.0], 1200);
        has("enviar", [50.0, 240.0, 150.0, 270.0], 400);
        // Una casilla sin marcar y un radio sin elegir solo tienen el borde: mucho menos que las marcadas.
        let off = ink(&pm, 2.0, [90.0, 450.0, 110.0, 470.0]);
        let on = ink(&pm, 2.0, [50.0, 450.0, 70.0, 470.0]);
        assert!(
            on > off,
            "{label}: la casilla marcada debe tener más tinta que la vacía ({on} vs {off})"
        );
    }

    // El texto formateado y el crudo se dibujan distinto: el campo de precio vivo (1.234,50 €) no puede ser idéntico a
    // dibujar «1234.5» tal cual; basta con que la tinta del campo supere la de un valor de seis caracteres.
    let live = render(&doc, 2.0);
    assert!(
        ink(&live, 2.0, wide(710.0)) > ink(&live, 2.0, wide(745.0)),
        "«1.234,50 €» ocupa más que «3»"
    );
}
