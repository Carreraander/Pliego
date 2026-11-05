//! Pruebas de los campos nativos: se comparan con la capa sobre MuPDF (mismo guion) y MuPDF dibuja el resultado.

use super::PdfDocument as Native;
use super::form_fixture::form_doc;
use super::tests::qpdf_check;
use crate::{AnnotationColor, Colorspace, Document, Matrix, Rect, WidgetType, WriteOptions};

fn bytes_of(doc: &Native) -> Vec<u8> {
    let out = doc.to_bytes(&WriteOptions::new()).unwrap();
    if let Some((code, log)) = qpdf_check(&out, None) {
        assert_eq!(code, 0, "qpdf --check:\n{log}");
    }
    out
}

fn mupdf_of(doc: &Native) -> crate::PdfDocument {
    crate::PdfDocument::from_bytes(&doc.to_bytes(&WriteOptions::new()).unwrap()).unwrap()
}

/// Lo observable de cada campo: tipo, nombre, indicadores, valor, etiqueta, solo lectura, firmado y estado `/AS`.
fn describe_native(doc: &Native) -> Vec<String> {
    let page = doc.load_pdf_page(0).unwrap();
    page.widgets()
        .map(|w| {
            let a = w.annotation().object().get_dict("AS").unwrap();
            let state = a.map(|a| String::from_utf8_lossy(&a.as_name().unwrap()).into_owned());
            format!(
                "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
                w.r#type().unwrap(),
                w.name().unwrap(),
                w.field_type_name().unwrap(),
                w.field_flags().unwrap(),
                w.value().unwrap(),
                w.label().unwrap(),
                w.is_readonly().unwrap(),
                w.is_signed().unwrap(),
                state
            )
        })
        .collect()
}

fn describe_mupdf(doc: &crate::PdfDocument) -> Vec<String> {
    let page = doc.load_pdf_page(0).unwrap();
    page.widgets()
        .map(|w| {
            let a = w
                .annotation()
                .object()
                .resolve()
                .unwrap()
                .unwrap()
                .get_dict("AS")
                .unwrap();
            let state = a.map(|a| String::from_utf8_lossy(&a.as_name().unwrap()).into_owned());
            format!(
                "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
                w.r#type().unwrap(),
                w.name().unwrap(),
                w.field_type_name().unwrap(),
                w.field_flags().unwrap(),
                w.value().unwrap(),
                w.label().unwrap(),
                w.is_readonly().unwrap(),
                w.is_signed().unwrap(),
                state
            )
        })
        .collect()
}

const SCRIPT: [(&str, &str); 9] = [
    ("nombre", "Ñandú"),
    ("acepto", "Si"),
    ("acepto", "Off"),
    ("acepto", "Si"),
    ("color", "rojo"),
    ("color", "verde"),
    ("pais", "FR"),
    ("lista", "b"),
    ("nombre", ""),
];

#[test]
fn reading_fields_matches_mupdf() {
    let doc = form_doc();
    let m = mupdf_of(&doc);
    assert_eq!(describe_native(&doc), describe_mupdf(&m));
    let types: Vec<_> = doc
        .load_pdf_page(0)
        .unwrap()
        .widgets()
        .map(|w| w.r#type().unwrap())
        .collect();
    assert_eq!(
        types,
        [
            WidgetType::Text,
            WidgetType::Checkbox,
            WidgetType::RadioButton,
            WidgetType::RadioButton,
            WidgetType::Combobox,
            WidgetType::Listbox,
            WidgetType::Button
        ]
    );
    assert!(doc.has_acro_form().unwrap());
}

#[test]
fn set_value_and_reset_match_mupdf() {
    let mut native = form_doc();
    let mut mupdf = mupdf_of(&native);
    for (name, value) in SCRIPT {
        let page = native.load_pdf_page(0).unwrap();
        for mut w in page.widgets().filter(|w| w.name().unwrap().as_deref() == Some(name)) {
            assert!(w.set_value(&mut native, value, true).unwrap(), "{name}={value}");
        }
        let page = mupdf.load_pdf_page(0).unwrap();
        for mut w in page.widgets().filter(|w| w.name().unwrap().as_deref() == Some(name)) {
            assert!(w.set_value(&mut mupdf, value, true).unwrap());
        }
        assert_eq!(describe_native(&native), describe_mupdf(&mupdf), "tras {name}={value}");
    }
    // Restablecer todos.
    let page = native.load_pdf_page(0).unwrap();
    for mut w in page.widgets() {
        w.reset(&mut native).unwrap();
    }
    let page = mupdf.load_pdf_page(0).unwrap();
    for mut w in page.widgets() {
        w.reset(&mut mupdf).unwrap();
    }
    // Diferencia deliberada: MuPDF no borra el `/V` de un grupo de radios al restablecer (deja «verde» aunque ningún
    // botón esté marcado); el modelo nativo sí.
    let (n, m) = (describe_native(&native), describe_mupdf(&mupdf));
    for (i, (a, b)) in n.iter().zip(&m).enumerate() {
        if i == 2 || i == 3 {
            assert!(
                a.contains("Some(\"\") Some(\"color\")") && b.contains("Some(\"verde\")"),
                "{a}\n{b}"
            );
        } else {
            assert_eq!(a, b, "tras restablecer");
        }
    }
    // El valor por defecto del texto vuelve.
    assert_eq!(
        native
            .load_pdf_page(0)
            .unwrap()
            .widgets()
            .next()
            .unwrap()
            .value()
            .unwrap()
            .as_deref(),
        Some("inicial")
    );
}

#[test]
fn unknown_toggle_state_and_buttons_are_rejected() {
    let mut doc = form_doc();
    let page = doc.load_pdf_page(0).unwrap();
    let mut ws: Vec<_> = page.widgets().collect();
    assert!(
        !ws[1].set_value(&mut doc, "Quizás", true).unwrap(),
        "la casilla no tiene ese estado"
    );
    assert!(
        !ws[2].set_value(&mut doc, "azul", true).unwrap(),
        "el radio no tiene ese estado"
    );
    assert!(!ws[6].set_value(&mut doc, "x", true).unwrap(), "botón de acción");
    assert_eq!(ws[1].value().unwrap().as_deref(), Some("Off"));
}

fn px(bytes: &[u8]) -> (Vec<u8>, usize) {
    let d = Document::from_bytes(bytes, "application/pdf").unwrap();
    let pix = d
        .load_page(0)
        .unwrap()
        .to_pixmap(&Matrix::IDENTITY, Colorspace::device_rgb(), false, true)
        .unwrap();
    (pix.samples().to_vec(), pix.stride())
}

/// Píxeles no blancos en la región de un campo (en espacio de página: y desde arriba; A4 = 595 × 842).
fn ink(samples: &(Vec<u8>, usize), r: Rect) -> usize {
    let mut n = 0;
    for y in (842.0 - r.y1).floor() as usize..(842.0 - r.y0).ceil() as usize {
        for x in r.x0.floor() as usize..r.x1.ceil() as usize {
            let i = y * samples.1 + x * 3;
            if samples.0[i..i + 3] != [255, 255, 255] {
                n += 1;
            }
        }
    }
    n
}

fn color_in(samples: &(Vec<u8>, usize), r: Rect, f: impl Fn(u8, u8, u8) -> bool) -> bool {
    for y in (842.0 - r.y1).floor() as usize..(842.0 - r.y0).ceil() as usize {
        for x in r.x0.floor() as usize..r.x1.ceil() as usize {
            let i = y * samples.1 + x * 3;
            if f(samples.0[i], samples.0[i + 1], samples.0[i + 2]) {
                return true;
            }
        }
    }
    false
}

#[test]
fn filled_fields_are_drawn() {
    let mut doc = form_doc();
    let before = px(&bytes_of(&doc));
    for (name, value) in [
        ("nombre", "Hola Ñandú"),
        ("acepto", "Si"),
        ("color", "rojo"),
        ("pais", "FR"),
        ("lista", "b"),
    ] {
        let page = doc.load_pdf_page(0).unwrap();
        for mut w in page.widgets().filter(|w| w.name().unwrap().as_deref() == Some(name)) {
            assert!(w.set_value(&mut doc, value, true).unwrap());
        }
    }
    // El botón de acción no tiene apariencia en el archivo: se genera aquí con el motor nativo (si no, sería MuPDF
    // quien lo dibujara al abrir y el resultado dependería de la plataforma y de sus fuentes del sistema).
    let page = doc.load_pdf_page(0).unwrap();
    for mut w in page
        .widgets()
        .filter(|w| w.name().unwrap().as_deref() == Some("enviar"))
    {
        assert!(w.update().unwrap());
    }
    let bytes = bytes_of(&doc);
    if let Ok(dir) = std::env::var("PLIEGO_DUMP_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(format!("{dir}/formulario.pdf"), &bytes).unwrap();
        let d = Document::from_bytes(&bytes, "application/pdf").unwrap();
        let pix = d
            .load_page(0)
            .unwrap()
            .to_pixmap(&Matrix::new_scale(2.0, 2.0), Colorspace::device_rgb(), false, true)
            .unwrap();
        std::fs::write(format!("{dir}/formulario.png"), pix.encode_png().unwrap()).unwrap();
    }
    let after = px(&bytes);
    // Texto del campo: más tinta que antes (que solo tenía el borde).
    assert!(
        ink(&after, Rect::new(52.0, 702.0, 248.0, 718.0)) > ink(&before, Rect::new(52.0, 702.0, 248.0, 718.0)) + 50
    );
    // Casilla marcada: tinta negra en el centro; antes, nada.
    assert_eq!(ink(&before, Rect::new(53.0, 653.0, 67.0, 667.0)), 0);
    assert!(ink(&after, Rect::new(53.0, 653.0, 67.0, 667.0)) > 10);
    // Radio «rojo» encendido y «verde» apagado.
    assert!(color_in(&after, Rect::new(54.0, 604.0, 66.0, 616.0), |r, g, b| r > 200
        && g < 60
        && b < 60));
    assert!(!color_in(&after, Rect::new(84.0, 604.0, 96.0, 616.0), |r, g, b| g
        > 200
        && r < 60
        && b < 60));
    // Combo con «FR», lista con la opción «Beta» resaltada.
    assert!(ink(&after, Rect::new(52.0, 552.0, 248.0, 568.0)) > 50);
    assert!(color_in(&after, Rect::new(52.0, 450.0, 248.0, 530.0), |r, g, b| r
        < 170
        && g > 170
        && b > 200));
    // Botón de acción con su rótulo y fondo gris (0,8 → ~204; MuPDF lo reparte con ±1 entre canales).
    assert!(color_in(&after, Rect::new(52.0, 402.0, 148.0, 428.0), |r, g, b| (r
        > 190
        && r < 215)
        && r.abs_diff(g) <= 2
        && r.abs_diff(b) <= 2));
    assert!(ink(&after, Rect::new(52.0, 402.0, 148.0, 428.0)) > 1000);
}

#[test]
fn bake_flattens_widgets_and_annotations() {
    let mut doc = form_doc();
    let page = doc.load_pdf_page(0).unwrap();
    for mut w in page
        .widgets()
        .filter(|w| w.name().unwrap().as_deref() == Some("nombre"))
    {
        w.set_value(&mut doc, "Texto fijo", true).unwrap();
    }
    for mut w in page
        .widgets()
        .filter(|w| w.name().unwrap().as_deref() == Some("acepto"))
    {
        w.set_value(&mut doc, "Si", true).unwrap();
    }
    let mut page = doc.load_pdf_page(0).unwrap();
    let mut hl = page
        .add_highlight_annotation(&[crate::Quad::new(
            crate::Point::new(300.0, 100.0),
            crate::Point::new(400.0, 100.0),
            crate::Point::new(300.0, 120.0),
            crate::Point::new(400.0, 120.0),
        )])
        .unwrap();
    hl.set_color(AnnotationColor::Rgb {
        red: 0.0,
        green: 1.0,
        blue: 1.0,
    })
    .unwrap();
    hl.update().unwrap();
    let with_fields = px(&bytes_of(&doc));

    // Solo campos: el resaltado se queda como anotación.
    let mut a = Native::from_bytes(&bytes_of(&doc)).unwrap();
    a.bake(false, true).unwrap();
    let a_bytes = bytes_of(&a);
    let a = Native::from_bytes(&a_bytes).unwrap();
    let p = a.load_pdf_page(0).unwrap();
    assert_eq!(p.widgets().count(), 0);
    assert_eq!(p.annotations().count(), 1);
    assert!(!a.has_acro_form().unwrap());
    let flat = px(&a_bytes);
    assert!(
        ink(&flat, Rect::new(52.0, 702.0, 248.0, 718.0)) > 50,
        "el texto quedó dibujado"
    );
    // Aplanado ≈ aspecto original (misma tinta en el campo, con el mismo motor de dibujo).
    let (x, y) = (
        ink(&flat, Rect::new(52.0, 702.0, 248.0, 718.0)),
        ink(&with_fields, Rect::new(52.0, 702.0, 248.0, 718.0)),
    );
    assert!(x.abs_diff(y) <= y / 20 + 5, "{x} frente a {y}");
    assert!(ink(&flat, Rect::new(53.0, 653.0, 67.0, 667.0)) > 10, "casilla marcada");

    // Con anotaciones: el resaltado también se funde.
    let mut b = Native::from_bytes(&bytes_of(&doc)).unwrap();
    b.bake(true, false).unwrap();
    let b_bytes = bytes_of(&b);
    let b = Native::from_bytes(&b_bytes).unwrap();
    let p = b.load_pdf_page(0).unwrap();
    assert_eq!(p.annotations().count(), 0);
    assert_eq!(p.widgets().count(), 7, "los campos no se tocan");
    let flat = px(&b_bytes);
    assert!(color_in(&flat, Rect::new(300.0, 722.0, 400.0, 742.0), |r, g, b| r
        < 100
        && g > 200
        && b > 200));
}

#[test]
fn delete_widget_and_add_signature_field() {
    let mut doc = form_doc();
    let mut page = doc.load_pdf_page(0).unwrap();
    let w = page
        .widgets()
        .find(|w| w.name().unwrap().as_deref() == Some("pais"))
        .unwrap();
    page.delete_widget(w).unwrap();
    assert_eq!(page.widgets().count(), 6);
    // Un radio con dos botones: al borrar uno sigue el campo; al borrar el último desaparece del formulario.
    for _ in 0..2 {
        let w = page
            .widgets()
            .find(|w| w.name().unwrap().as_deref() == Some("color"))
            .unwrap();
        page.delete_widget(w).unwrap();
    }
    assert!(page.widgets().all(|w| w.name().unwrap().as_deref() != Some("color")));
    let af = doc.catalog().unwrap().get_dict("AcroForm").unwrap().unwrap();
    assert_eq!(af.get_dict("Fields").unwrap().unwrap().len().unwrap(), 4);

    let mut sig = page.add_signature_widget("firma1").unwrap();
    assert_eq!(sig.r#type().unwrap(), WidgetType::Signature);
    assert!(!sig.is_signed().unwrap());
    assert!(!sig.set_value(&mut doc, "x", true).unwrap());
    let bytes = bytes_of(&doc);
    let again = Native::from_bytes(&bytes).unwrap();
    let names: Vec<_> = again
        .load_pdf_page(0)
        .unwrap()
        .widgets()
        .map(|w| w.name().unwrap().unwrap())
        .collect();
    assert!(
        names.contains(&"firma1".to_string()) && !names.contains(&"pais".to_string()),
        "{names:?}"
    );
    // MuPDF lo ve igual.
    let m = crate::PdfDocument::from_bytes(&bytes).unwrap();
    assert_eq!(describe_mupdf(&m).len(), names.len());
}

#[test]
fn fields_survive_page_copy_and_toggle_without_appearance_gets_one() {
    // Una casilla sin /AP: `update()` le crea los estados Yes/Off y se ve al marcarla.
    let mut doc = Native::new();
    let page = doc.new_page(crate::Size::A4).unwrap();
    let pid = page.object().as_indirect().unwrap();
    let c = doc
        .new_object_from_str(&format!(
            "<< /Type /Annot /Subtype /Widget /FT /Btn /T (c) /V /Off /AS /Off /Rect [100 100 120 120] /F 4 /P {pid} 0 R /MK << /BC [0] >> >>"
        ))
        .unwrap();
    let c = doc.add_object(&c).unwrap();
    let annots = doc
        .new_object_from_str(&format!("[{} 0 R]", c.as_indirect().unwrap()))
        .unwrap();
    page.object().dict_put("Annots", annots).unwrap();
    let af = doc
        .new_object_from_str(&format!("<< /Fields [{} 0 R] >>", c.as_indirect().unwrap()))
        .unwrap();
    let af = doc.add_object(&af).unwrap();
    doc.catalog().unwrap().dict_put("AcroForm", af).unwrap();

    let mut w = page.widgets().next().unwrap();
    assert!(w.update().unwrap());
    assert!(!w.update().unwrap(), "ya tiene estados");
    assert!(w.set_value(&mut doc, "Yes", true).unwrap());
    let bytes = bytes_of(&doc);
    let s = px(&bytes);
    assert!(ink(&s, Rect::new(102.0, 102.0, 118.0, 118.0)) > 8);
    let m = crate::PdfDocument::from_bytes(&bytes).unwrap();
    assert!(describe_mupdf(&m)[0].contains("\"Yes\""), "{:?}", describe_mupdf(&m));
}
