//! Pruebas del JavaScript de formularios sobre el modelo nativo (motor `pliego-formjs`).

use super::PdfDocument as Native;
use super::object::Object;
use super::tests::qpdf_check;
use crate::{Size, WriteOptions};

fn add(doc: &mut Native, src: &str) -> Object {
    let o = doc.new_object_from_str(src).unwrap();
    doc.add_object(&o).unwrap()
}

fn id(o: &Object) -> i32 {
    o.as_indirect().unwrap()
}

/// Cadena PDF hexadecimal con el código (evita escapar paréntesis y comillas). Con acentos, UTF-16BE con BOM, como
/// escriben los editores de PDF.
fn hex(s: &str) -> String {
    let bytes: Vec<u8> = if s.is_ascii() {
        s.bytes().collect()
    } else {
        let mut b = vec![0xFE, 0xFF];
        s.encode_utf16().for_each(|u| b.extend_from_slice(&u.to_be_bytes()));
        b
    };
    let body: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("<{body}>")
}

fn js(code: &str) -> String {
    format!("<< /S /JavaScript /JS {} >>", hex(code))
}

const DNI_JS: &str = "(function(){var v=String(event.value).toUpperCase().replace(/[\\s-]/g,'');if(v==='')return;\
var L='TRWAGMYFPDXBNJZSQVHLCKE';var m=/^([XYZ]|\\d)(\\d{7})([A-Z])$/.exec(v);var ok=false;\
if(m){var n=m[1]==='X'?'0':m[1]==='Y'?'1':m[1]==='Z'?'2':m[1];ok=L.charAt(parseInt(n+m[2],10)%23)===m[3];}\
if(!ok){event.rc=false;app.alert('El DNI o NIE no es válido.');}})();";

struct Spec<'a> {
    name: &'a str,
    extra: String,
}

/// Documento con campos de texto en una página: cada uno con su `extra` (acciones, valor...). El orden de cálculo
/// `calc` se escribe en `/CO`. Devuelve el documento y los ids de los campos.
fn doc_with(specs: &[Spec<'_>], calc: &[&str], names_js: Option<&str>) -> Native {
    let mut doc = Native::new();
    let page = doc.new_page(Size::A4).unwrap();
    let pid = id(&page.object());
    let helv = add(
        &mut doc,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    let mut ids = Vec::new();
    for (i, s) in specs.iter().enumerate() {
        let y = 700 - 40 * i as i32;
        let o = add(
            &mut doc,
            &format!(
                "<< /Type /Annot /Subtype /Widget /FT /Tx /T ({}) /Rect [50 {y} 250 {}] /F 4 /P {pid} 0 R \
                 /DA (/Helv 12 Tf 0 g) /MK << /BC [0] >> {} >>",
                s.name,
                y + 20,
                s.extra
            ),
        );
        ids.push(o);
    }
    let annots = ids
        .iter()
        .map(|o| format!("{} 0 R", id(o)))
        .collect::<Vec<_>>()
        .join(" ");
    let mut p = page.object();
    p.dict_put("Annots", doc.new_object_from_str(&format!("[{annots}]")).unwrap())
        .unwrap();
    let co = calc
        .iter()
        .map(|n| {
            let i = specs.iter().position(|s| s.name == *n).unwrap();
            format!("{} 0 R", id(&ids[i]))
        })
        .collect::<Vec<_>>()
        .join(" ");
    let acro = add(
        &mut doc,
        &format!(
            "<< /Fields [{annots}] /CO [{co}] /DA (/Helv 0 Tf 0 g) /DR << /Font << /Helv {} 0 R >> >> >>",
            id(&helv)
        ),
    );
    doc.catalog().unwrap().dict_put("AcroForm", acro).unwrap();
    if let Some(code) = names_js {
        let names = doc
            .new_object_from_str(&format!("<< /JavaScript << /Names [(util) {}] >> >>", js(code)))
            .unwrap();
        doc.catalog().unwrap().dict_put("Names", names).unwrap();
    }
    doc
}

fn reopen(doc: &Native) -> Native {
    let bytes = doc.to_bytes(&WriteOptions::new()).unwrap();
    if let Some((code, log)) = qpdf_check(&bytes, None) {
        assert_eq!(code, 0, "qpdf --check:\n{log}");
    }
    Native::from_bytes(&bytes).unwrap()
}

fn set(doc: &mut Native, name: &str, v: &str) -> bool {
    let page = doc.load_pdf_page(0).unwrap();
    let mut w = page
        .widgets()
        .find(|w| w.name().unwrap().as_deref() == Some(name))
        .unwrap();
    w.set_value(doc, v, false).unwrap()
}

fn value(doc: &Native, name: &str) -> Option<String> {
    let page = doc.load_pdf_page(0).unwrap();
    page.widgets()
        .find(|w| w.name().unwrap().as_deref() == Some(name))
        .unwrap()
        .value()
        .unwrap()
}

/// Contenido del `/AP /N` del campo (lo que se dibuja).
fn ap_text(doc: &Native, name: &str) -> String {
    let page = doc.load_pdf_page(0).unwrap();
    let w = page
        .widgets()
        .find(|w| w.name().unwrap().as_deref() == Some(name))
        .unwrap();
    let n = w
        .annotation()
        .object()
        .get_dict("AP")
        .unwrap()
        .unwrap()
        .get_dict("N")
        .unwrap()
        .unwrap();
    String::from_utf8_lossy(&n.read_stream().unwrap()).into_owned()
}

fn aa(key: &str, code: &str) -> String {
    format!("/AA << /{key} {} >>", js(code))
}

#[test]
fn js_is_off_until_enabled() {
    let mut doc = doc_with(
        &[Spec {
            name: "edad",
            extra: aa("V", "AFRange_Validate(true, 0, true, 120);"),
        }],
        &[],
        None,
    );
    assert!(doc.is_js_supported().unwrap());
    assert!(
        set(&mut doc, "edad", "200"),
        "sin enable_js no se ejecuta ningún script"
    );
    doc.enable_js().unwrap();
    assert!(!set(&mut doc, "edad", "300"));
    assert_eq!(value(&doc, "edad").as_deref(), Some("200"));
    // `ignore_trigger_events` salta los scripts.
    let page = doc.load_pdf_page(0).unwrap();
    let mut w = page.widgets().next().unwrap();
    assert!(w.set_value(&mut doc, "300", true).unwrap());
    doc.disable_js().unwrap();
    assert!(set(&mut doc, "edad", "400"));
}

#[test]
fn validation_scripts_reject_values_and_report_alerts() {
    let doc = doc_with(
        &[
            Spec {
                name: "edad",
                extra: aa("V", "AFRange_Validate(true, 0, true, 120);"),
            },
            Spec {
                name: "dni",
                extra: aa("V", DNI_JS),
            },
            Spec {
                name: "corto",
                extra: aa("V", "if (String(event.value).length > 3) event.rc = false;"),
            },
        ],
        &[],
        None,
    );
    let mut doc = reopen(&doc);
    doc.enable_js().unwrap();
    assert!(set(&mut doc, "edad", "50"));
    assert!(!set(&mut doc, "edad", "200"));
    assert!(!set(&mut doc, "edad", "-1"));
    assert_eq!(value(&doc, "edad").as_deref(), Some("50"));
    assert!(set(&mut doc, "dni", "12345678Z"));
    assert!(set(&mut doc, "dni", "X1234567L"));
    assert!(!set(&mut doc, "dni", "12345678A"));
    assert!(set(&mut doc, "corto", "abc"));
    assert!(!set(&mut doc, "corto", "abcd"));
    let msgs = doc.take_js_messages();
    assert!(
        msgs.iter()
            .any(|m| matches!(m, super::JsMessage::Alert { text, .. } if text == "El DNI o NIE no es válido."))
    );
    assert!(doc.take_js_messages().is_empty(), "los avisos se entregan una vez");
}

fn calc(op: &str, fields: &[&str]) -> String {
    let list: Vec<String> = fields.iter().map(|f| format!("'{f}'")).collect();
    format!("AFSimple_Calculate('{op}', new Array({}));", list.join(","))
}

#[test]
fn calculations_run_in_the_calculation_order() {
    let specs = [
        Spec {
            name: "a",
            extra: String::new(),
        },
        Spec {
            name: "b",
            extra: String::new(),
        },
        // `doble` se declara antes que `total`, pero /CO las ordena al revés.
        Spec {
            name: "doble",
            extra: aa("C", &calc("SUM", &["total", "total"])),
        },
        Spec {
            name: "total",
            extra: aa(
                "C",
                &format!(
                    "{} event.value = Math.round(event.value * 100) / 100;",
                    calc("SUM", &["a", "b"])
                ),
            ),
        },
        Spec {
            name: "media",
            extra: aa("C", &calc("AVG", &["a", "b"])),
        },
    ];
    let doc = doc_with(&specs, &["total", "doble", "media"], None);
    let mut doc = reopen(&doc);
    doc.enable_js().unwrap();
    assert!(set(&mut doc, "a", "1.1"));
    assert!(set(&mut doc, "b", "2.2"));
    assert_eq!(value(&doc, "total").as_deref(), Some("3.3"));
    assert_eq!(value(&doc, "doble").as_deref(), Some("6.6"));
    assert_eq!(value(&doc, "media").as_deref(), Some("1.65"));
    assert!(
        ap_text(&doc, "total").contains("3.3"),
        "la apariencia se regenera con el cálculo"
    );
    let out = reopen(&doc);
    assert_eq!(value(&out, "doble").as_deref(), Some("6.6"), "el resultado se guarda");
}

#[test]
fn calculate_form_recomputes_everything() {
    let specs = [
        Spec {
            name: "a",
            extra: "/V (4)".into(),
        },
        Spec {
            name: "b",
            extra: "/V (6)".into(),
        },
        Spec {
            name: "t",
            extra: aa("C", &calc("SUM", &["a", "b"])),
        },
    ];
    let mut doc = reopen(&doc_with(&specs, &["t"], None));
    doc.calculate_form().unwrap();
    assert_ne!(value(&doc, "t").as_deref(), Some("10"), "sin enable_js no calcula");
    doc.enable_js().unwrap();
    doc.calculate_form().unwrap();
    assert_eq!(value(&doc, "t").as_deref(), Some("10"));
}

#[test]
fn format_scripts_change_what_is_drawn_not_the_value() {
    let specs = [Spec {
        name: "importe",
        extra: format!(
            "{} /AA << /F {} /K {} >>",
            "",
            js("AFNumber_Format(2, 2, 0, 0, ' €', false);"),
            js("AFNumber_Keystroke(2, 2, 0, 0, ' €', false);")
        ),
    }];
    let mut doc = reopen(&doc_with(&specs, &[], None));
    doc.enable_js().unwrap();
    assert!(set(&mut doc, "importe", "1234.5"));
    assert_eq!(
        value(&doc, "importe").as_deref(),
        Some("1234.5"),
        "/V guarda lo tecleado"
    );
    let ap = ap_text(&doc, "importe");
    assert!(
        ap.contains("1.234,50"),
        "la apariencia muestra el valor formateado: {ap}"
    );
    assert!(!ap.contains("1234.5"));
    assert!(
        !set(&mut doc, "importe", "12abc"),
        "el tecleo rechaza lo que no es número"
    );
    assert_eq!(value(&doc, "importe").as_deref(), Some("1234.5"));
    // Al guardar y abrir de nuevo, la apariencia sigue siendo la formateada y el valor, el tecleado.
    let mut again = reopen(&doc);
    assert!(ap_text(&again, "importe").contains("1.234,50"));
    assert_eq!(value(&again, "importe").as_deref(), Some("1234.5"));
    // Activar el JavaScript en un documento ya relleno prepara el texto formateado para futuros dibujos.
    again.enable_js().unwrap();
    let page = again.load_pdf_page(0).unwrap();
    let mut w = page.widgets().next().unwrap();
    w.update().unwrap();
    assert!(ap_text(&again, "importe").contains("1.234,50"));
}

#[test]
fn scripts_change_properties_and_other_fields() {
    let specs = [
        Spec {
            name: "tiene",
            extra: aa(
                "V",
                "var d = this.getField('detalle'); d.display = (event.value == 'si') ? display.visible : display.hidden; \
                 d.fillColor = color.yellow; d.readonly = (event.value != 'si'); d.textSize = 9; d.value = 'x' + event.value;",
            ),
        },
        Spec {
            name: "detalle",
            extra: String::new(),
        },
    ];
    let mut doc = reopen(&doc_with(&specs, &[], None));
    doc.enable_js().unwrap();
    assert!(set(&mut doc, "tiene", "no"));
    let doc = reopen(&doc);
    let page = doc.load_pdf_page(0).unwrap();
    let w = page
        .widgets()
        .find(|w| w.name().unwrap().as_deref() == Some("detalle"))
        .unwrap();
    let o = w.annotation().object();
    assert_eq!(o.get_dict("F").unwrap().unwrap().as_int().unwrap() & 2, 2, "oculto");
    assert!(w.is_readonly().unwrap());
    let mk = o.get_dict("MK").unwrap().unwrap();
    assert_eq!(
        mk.get_dict("BG").unwrap().unwrap().len().unwrap(),
        4,
        "fondo CMYK amarillo"
    );
    assert_eq!(
        mk.get_dict("BC").unwrap().unwrap().len().unwrap(),
        1,
        "el borde se conserva"
    );
    assert!(o.get_dict("DA").unwrap().unwrap().as_string().unwrap().contains("9 Tf"));
    assert_eq!(value(&doc, "detalle").as_deref(), Some("xno"));
}

#[test]
fn document_level_scripts_and_reset() {
    let specs = [
        Spec {
            name: "x",
            extra: "/V (4) /DV (7)".into(),
        },
        Spec {
            name: "y",
            extra: aa("C", "event.value = doble(Number(this.getField('x').value));"),
        },
    ];
    let mut doc = reopen(&doc_with(&specs, &["y"], Some("function doble(n) { return n * 2; }")));
    doc.enable_js().unwrap();
    assert!(set(&mut doc, "x", "21"));
    assert_eq!(value(&doc, "y").as_deref(), Some("42"));
    let page = doc.load_pdf_page(0).unwrap();
    let mut w = page.widgets().next().unwrap();
    w.reset(&mut doc).unwrap();
    assert_eq!(value(&doc, "x").as_deref(), Some("7"));
    assert_eq!(value(&doc, "y").as_deref(), Some("14"), "restablecer recalcula");
}

#[test]
fn script_errors_are_logged_not_fatal() {
    let specs = [Spec {
        name: "a",
        extra: aa("V", "this.getField('no existe').value = 1;"),
    }];
    let mut doc = reopen(&doc_with(&specs, &[], Some("function (")));
    doc.enable_js().unwrap();
    assert!(set(&mut doc, "a", "x"));
    let msgs = doc.take_js_messages();
    assert_eq!(msgs.len(), 2, "{msgs:?}");
    assert!(
        msgs.iter()
            .all(|m| matches!(m, super::JsMessage::Console(t) if t.contains("rror")))
    );
}

#[test]
fn fixture_form_with_every_field_type() {
    use super::form_fixture::form_doc;
    let mut doc = form_doc();
    doc.enable_js().unwrap();
    let page = doc.load_pdf_page(0).unwrap();
    let by_name = |n: &str| {
        page.widgets()
            .find(|w| w.name().unwrap().as_deref() == Some(n))
            .unwrap()
    };
    let (mut chk, mut pais, mut texto) = (by_name("acepto"), by_name("pais"), by_name("nombre"));
    assert!(chk.set_value(&mut doc, "Si", false).unwrap());
    assert!(!chk.set_value(&mut doc, "Quizá", false).unwrap());
    assert!(pais.set_value(&mut doc, "FR", false).unwrap());
    assert!(texto.set_value(&mut doc, "Ana", false).unwrap());
    let doc = reopen(&doc);
    assert_eq!(value(&doc, "acepto").as_deref(), Some("Si"));
    assert_eq!(value(&doc, "pais").as_deref(), Some("FR"));
    assert_eq!(value(&doc, "nombre").as_deref(), Some("Ana"));
}

#[test]
fn keystroke_filters_typing_per_field() {
    let specs = [
        Spec {
            name: "n",
            extra: aa("K", "AFNumber_Keystroke(2, 0, 0, 0, '', false);"),
        },
        Spec {
            name: "mayus",
            extra: aa("K", "if (!event.willCommit) event.change = event.change.toUpperCase();"),
        },
        Spec {
            name: "libre",
            extra: String::new(),
        },
    ];
    let mut doc = reopen(&doc_with(&specs, &[], None));
    let page = doc.load_pdf_page(0).unwrap();
    let get = |n: &str| {
        page.widgets()
            .find(|w| w.name().unwrap().as_deref() == Some(n))
            .unwrap()
    };
    let (mut n, mut mayus, mut libre) = (get("n"), get("mayus"), get("libre"));
    // Sin enable_js todo se acepta.
    assert_eq!(n.keystroke(&mut doc, "x", "", (0, 0)).unwrap(), (true, "x".into()));
    doc.enable_js().unwrap();
    assert_eq!(n.keystroke(&mut doc, "7", "12", (2, 2)).unwrap(), (true, "7".into()));
    assert!(!n.keystroke(&mut doc, "x", "12", (2, 2)).unwrap().0);
    assert_eq!(
        mayus.keystroke(&mut doc, "abc", "", (0, 0)).unwrap(),
        (true, "ABC".into())
    );
    assert_eq!(libre.keystroke(&mut doc, "q", "", (0, 0)).unwrap(), (true, "q".into()));
    assert!(
        value(&doc, "n").unwrap_or_default().is_empty(),
        "teclear no guarda el valor"
    );
}

fn page_with_button(doc: &mut Native) {
    let page = doc.load_pdf_page(0).unwrap();
    let pid = id(&page.object());
    let b = add(
        doc,
        &format!(
            "<< /Type /Annot /Subtype /Widget /FT /Btn /Ff 65536 /T (limpiar) /Rect [300 700 400 720] /F 4 \
             /P {pid} 0 R /MK << /CA (Limpiar) >> /A {} >>",
            js("this.getField('a').value = ''; this.getField('b').display = display.hidden; app.alert('Hecho', 3);")
        ),
    );
    let p = page.object();
    let annots = p.get_dict("Annots").unwrap().unwrap();
    let mut annots = annots;
    annots.array_push(b.clone()).unwrap();
    // Un widget que no cuelga de /AcroForm /Fields hace saltar a qpdf (versiones recientes).
    let mut fields = doc
        .catalog()
        .unwrap()
        .get_dict("AcroForm")
        .unwrap()
        .unwrap()
        .get_dict("Fields")
        .unwrap()
        .unwrap();
    fields.array_push(b).unwrap();
}

#[test]
fn button_actions_run_and_change_other_fields() {
    let specs = [
        Spec {
            name: "a",
            extra: "/V (algo)".into(),
        },
        Spec {
            name: "b",
            extra: String::new(),
        },
    ];
    let mut doc = doc_with(&specs, &[], None);
    page_with_button(&mut doc);
    let mut doc = reopen(&doc);
    let page = doc.load_pdf_page(0).unwrap();
    let mut btn = page
        .widgets()
        .find(|w| w.name().unwrap().as_deref() == Some("limpiar"))
        .unwrap();
    assert!(!btn.run_action(&mut doc, "U").unwrap(), "sin enable_js no hace nada");
    doc.enable_js().unwrap();
    assert!(btn.run_action(&mut doc, "U").unwrap());
    assert_eq!(value(&doc, "a").as_deref(), Some(""));
    let b = reopen(&doc);
    let page = b.load_pdf_page(0).unwrap();
    let w = page
        .widgets()
        .find(|w| w.name().unwrap().as_deref() == Some("b"))
        .unwrap();
    let f = w
        .annotation()
        .object()
        .get_dict("F")
        .unwrap()
        .unwrap()
        .as_int()
        .unwrap();
    assert_eq!(f & 2, 2, "el botón ocultó el campo b");
    assert!(
        doc.take_js_messages()
            .iter()
            .any(|m| matches!(m, super::JsMessage::Alert { text, icon: 3, .. } if text == "Hecho"))
    );
}

#[test]
fn document_and_page_events() {
    use super::JsDocEvent as E;
    let specs = [
        Spec {
            name: "x",
            extra: String::new(),
        },
        Spec {
            name: "log",
            extra: String::new(),
        },
    ];
    let doc = doc_with(&specs, &[], None);
    let mut cat = doc.catalog().unwrap();
    let fill = |what: &str| format!("this.getField('log').value = this.getField('log').value + '{what};';");
    cat.dict_put("OpenAction", doc.new_object_from_str(&js(&fill("abrir"))).unwrap())
        .unwrap();
    cat.dict_put(
        "AA",
        doc.new_object_from_str(&format!(
            "<< /WS {} /DS {} >>",
            js(&fill("guardar")),
            js(&fill("guardado"))
        ))
        .unwrap(),
    )
    .unwrap();
    let mut page = doc.load_pdf_page(0).unwrap().object();
    page.dict_put(
        "AA",
        doc.new_object_from_str(&format!("<< /O {} >>", js(&fill("pagina"))))
            .unwrap(),
    )
    .unwrap();
    let mut doc = reopen(&doc);
    assert!(!doc.run_document_event(E::Open).unwrap(), "sin enable_js no se ejecuta");
    doc.enable_js().unwrap();
    assert!(doc.run_document_event(E::Open).unwrap());
    assert!(doc.run_document_event(E::PageOpen(0)).unwrap());
    assert!(doc.run_document_event(E::WillSave).unwrap());
    assert!(!doc.run_document_event(E::WillPrint).unwrap(), "sin script");
    assert!(!doc.run_document_event(E::PageClose(0)).unwrap());
    assert!(!doc.run_document_event(E::PageOpen(9)).unwrap(), "página inexistente");
    assert!(doc.run_document_event(E::DidSave).unwrap());
    assert_eq!(value(&doc, "log").as_deref(), Some("abrir;pagina;guardar;guardado;"));
}
