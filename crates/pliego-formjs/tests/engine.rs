//! Pruebas del motor sobre un formulario en memoria.

use pliego_formjs::{Change, Config, Engine, Field, FieldKind, Form, Language, Message};

/// Fecha fija: 5 de noviembre de 2026, 12:00 UTC.
const NOW: f64 = 1_793_880_000_000.0;

fn cfg() -> Config {
    Config {
        language: Language::Es,
        now_ms: Some(NOW),
        ..Default::default()
    }
}

fn text(name: &str) -> Field {
    Field::new(name, FieldKind::Text)
}

fn with(mut f: Field, key: &str, js: &str) -> Field {
    f.scripts.insert(key.into(), js.into());
    f
}

fn engine(fields: Vec<Field>) -> Engine {
    Engine::new(
        Form {
            fields,
            page_count: 3,
            file_name: "formulario.pdf".into(),
            ..Default::default()
        },
        cfg(),
    )
}

fn value(e: &Engine, name: &str) -> String {
    e.form().field(name).unwrap().value.clone()
}

fn shown(e: &Engine, name: &str) -> String {
    e.form().field(name).unwrap().shown().to_string()
}

#[test]
fn eval_runs_plain_javascript() {
    let mut e = engine(vec![]);
    assert_eq!(e.eval("1 + 2").unwrap(), "3");
    assert_eq!(
        e.eval("/^[A-Z]{2}\\d{2}/.test('ES91') + ' ' + new Date(2026, 10, 5).getDate()")
            .unwrap(),
        "true 5"
    );
    assert_eq!(
        e.eval("[3,1,2].sort().join('-') + JSON.stringify({a:1})").unwrap(),
        "1-2-3{\"a\":1}"
    );
    assert!(e.eval("nope(").is_err());
    assert!(e.load_errors().is_empty());
}

#[test]
fn infinite_loops_are_aborted() {
    let mut e = engine(vec![with(text("a"), "K", "while (true) {}")]);
    let out = e.commit("a", "x");
    assert!(!out.errors.is_empty(), "el bucle infinito se corta y se anota");
    let mut e = engine(vec![]);
    assert!(e.eval("function f(){ return f(); } f()").is_err(), "recursión infinita");
}

#[test]
fn range_validation_uses_af_range_validate() {
    let mut e = engine(vec![with(text("edad"), "V", "AFRange_Validate(true, 0, true, 120);")]);
    assert!(e.commit("edad", "50").accepted);
    assert_eq!(value(&e, "edad"), "50");
    let out = e.commit("edad", "200");
    assert!(!out.accepted);
    assert!(out.changes.is_empty());
    assert!(matches!(&out.messages[0], Message::Alert { text, .. } if text.contains("120")));
    assert!(!e.commit("edad", "-1").accepted);
    assert!(!e.commit("edad", "abc").accepted);
    assert!(e.commit("edad", "").accepted, "vacío siempre pasa");
    assert_eq!(value(&e, "edad"), "", "el vacío sí se guarda");
    let mut e = engine(vec![with(text("n"), "V", "AFRange_Validate(true, 5, false, 0);")]);
    assert!(!e.commit("n", "4").accepted);
    assert!(e.commit("n", "5").accepted);
}

// Los mismos scripts que genera pliego-formdesign.
const DNI_JS: &str = "(function(){var v=String(event.value).toUpperCase().replace(/[\\s-]/g,'');if(v==='')return;\
var L='TRWAGMYFPDXBNJZSQVHLCKE';var m=/^([XYZ]|\\d)(\\d{7})([A-Z])$/.exec(v);var ok=false;\
if(m){var n=m[1]==='X'?'0':m[1]==='Y'?'1':m[1]==='Z'?'2':m[1];ok=L.charAt(parseInt(n+m[2],10)%23)===m[3];}\
if(!ok){event.rc=false;app.alert('El DNI o NIE no es válido.');}})();";

const IBAN_JS: &str = "(function(){var v=String(event.value).toUpperCase().replace(/\\s/g,'');if(v==='')return;\
var ok=/^[A-Z]{2}\\d{2}[A-Z0-9]{11,30}$/.test(v);if(ok){var r=v.substring(4)+v.substring(0,4);var rem=0;\
for(var i=0;i<r.length;i++){var c=r.charAt(i);var d=(c>='A'&&c<='Z')?String(c.charCodeAt(0)-55):c;\
for(var j=0;j<d.length;j++){rem=(rem*10+parseInt(d.charAt(j),10))%97;}}ok=rem===1;}\
if(!ok){event.rc=false;app.alert('El IBAN no es válido.');}})();";

#[test]
fn dni_and_iban_scripts() {
    let mut e = engine(vec![with(text("dni"), "V", DNI_JS), with(text("iban"), "V", IBAN_JS)]);
    for ok in ["12345678Z", "X1234567L", "12345678-z"] {
        assert!(e.commit("dni", ok).accepted, "{ok}");
    }
    for bad in ["12345678A", "1234"] {
        assert!(!e.commit("dni", bad).accepted, "{bad}");
    }
    assert!(e.commit("iban", "ES91 2100 0418 4502 0005 1332").accepted);
    assert!(!e.commit("iban", "ES91 2100 0418 4502 0005 1333").accepted);
    assert!(!e.commit("iban", "no es un iban").accepted);
    let out = e.commit("iban", "xx");
    assert!(matches!(&out.messages[0], Message::Alert { text, .. } if text == "El IBAN no es válido."));
}

#[test]
fn custom_validation_script_can_veto() {
    let mut e = engine(vec![with(
        text("corto"),
        "V",
        "if (String(event.value).length > 3) event.rc = false;",
    )]);
    assert!(e.commit("corto", "abc").accepted);
    assert!(!e.commit("corto", "abcd").accepted);
    assert_eq!(value(&e, "corto"), "abc");
}

fn calc(op: &str, fields: &[&str]) -> String {
    let list: Vec<String> = fields.iter().map(|f| format!("\"{f}\"")).collect();
    format!("AFSimple_Calculate(\"{op}\", new Array({}));", list.join(","))
}

#[test]
fn calculations_follow_the_calculation_order() {
    let form = Form {
        fields: vec![
            text("a"),
            text("b"),
            // `doble` depende de `total`; en el orden de cálculo va después.
            with(text("doble"), "C", &calc("SUM", &["total", "total"])),
            with(
                text("total"),
                "C",
                &format!(
                    "{} event.value = Math.round(event.value * 100) / 100;",
                    calc("SUM", &["a", "b"])
                ),
            ),
            with(text("media"), "C", &calc("AVG", &["a", "b"])),
            with(text("min"), "C", &calc("MIN", &["a", "b"])),
            with(text("max"), "C", &calc("MAX", &["a", "b"])),
            with(text("prod"), "C", &calc("PRD", &["a", "b"])),
        ],
        calc_order: ["total", "doble", "media", "min", "max", "prod"]
            .map(String::from)
            .to_vec(),
        ..Default::default()
    };
    let mut e = Engine::new(form, cfg());
    let out = e.commit("a", "1.1");
    assert!(out.accepted);
    assert_eq!(value(&e, "total"), "1.1");
    let out = e.commit("b", "2.2");
    assert_eq!(value(&e, "total"), "3.3", "redondeado");
    assert_eq!(value(&e, "doble"), "6.6");
    assert_eq!(value(&e, "media"), "1.65");
    assert_eq!(value(&e, "min"), "1.1");
    assert_eq!(value(&e, "max"), "2.2");
    assert_eq!(value(&e, "prod"), "2.42");
    assert!(out.changed_values().contains(&"doble"));
    e.commit("b", "-3");
    assert_eq!(value(&e, "prod"), "-3.3");
}

#[test]
fn calculation_without_order_still_runs() {
    let mut e = engine(vec![
        text("x"),
        with(text("y"), "C", "event.value = Number(this.getField('x').value) * 2;"),
    ]);
    e.commit("x", "21");
    assert_eq!(value(&e, "y"), "42");
    // Un campo vacío y uno con texto no numérico no rompen la suma.
    let mut e = engine(vec![
        text("a"),
        text("b"),
        with(text("t"), "C", &calc("SUM", &["a", "b"])),
    ]);
    e.commit("a", "5");
    e.commit("b", "hola");
    assert_eq!(value(&e, "t"), "5");
}

#[test]
fn number_format_sets_display_text_but_keeps_value() {
    let mut e = engine(vec![with(
        with(text("importe"), "F", "AFNumber_Format(2, 2, 0, 0, \" €\", false);"),
        "K",
        "AFNumber_Keystroke(2, 2, 0, 0, \" €\", false);",
    )]);
    let out = e.commit("importe", "1234.5");
    assert!(out.accepted);
    assert_eq!(value(&e, "importe"), "1234.5");
    assert_eq!(shown(&e, "importe"), "1.234,50 €");
    assert_eq!(out.display_of("importe"), Some(&Some("1.234,50 €".to_string())));
    assert!(e.commit("importe", "1.234,50").accepted);
    assert_eq!(shown(&e, "importe"), "1.234,50 €");
    assert!(!e.commit("importe", "12abc").accepted, "no es un número");
    assert_eq!(shown(&e, "importe"), "1.234,50 €", "el rechazo no toca lo anterior");
    assert!(e.commit("importe", "").accepted);
    assert_eq!(shown(&e, "importe"), "");
    // Moneda delante y negativos entre paréntesis.
    let mut e = engine(vec![with(text("n"), "F", "AFNumber_Format(0, 0, 2, 0, '$', true);")]);
    e.commit("n", "-1234");
    assert_eq!(shown(&e, "n"), "($1,234)");
}

#[test]
fn negative_style_changes_text_color() {
    let mut e = engine(vec![with(text("n"), "F", "AFNumber_Format(2, 1, 1, 0, '', false);")]);
    let out = e.commit("n", "-5");
    assert_eq!(shown(&e, "n"), "-5.00");
    assert!(out.changes.contains(&Change::Prop {
        field: "n".into(),
        prop: "textColor".into()
    }));
    assert_eq!(
        e.form().field("n").unwrap().text_color,
        pliego_formjs::Color::Rgb(1.0, 0.0, 0.0)
    );
    e.commit("n", "5");
    assert_eq!(e.form().field("n").unwrap().text_color, pliego_formjs::Color::Gray(0.0));
}

#[test]
fn percent_date_time_and_special_formats() {
    let mut e = engine(vec![
        with(
            with(text("pct"), "F", "AFPercent_Format(1, 2);"),
            "K",
            "AFPercent_Keystroke(1, 2);",
        ),
        with(
            with(text("fecha"), "F", "AFDate_FormatEx('dd/mm/yyyy');"),
            "K",
            "AFDate_KeystrokeEx('dd/mm/yyyy');",
        ),
        with(
            with(text("larga"), "F", "AFDate_FormatEx('d mmmm yyyy');"),
            "K",
            "AFDate_KeystrokeEx('d mmmm yyyy');",
        ),
        with(
            with(text("hora"), "F", "AFTime_FormatEx('HH:MM');"),
            "K",
            "AFTime_KeystrokeEx('HH:MM');",
        ),
        with(
            with(text("cp"), "F", "AFSpecial_Format(0);"),
            "K",
            "AFSpecial_Keystroke(0);",
        ),
        with(
            with(text("tel"), "F", "AFSpecial_Format(2);"),
            "K",
            "AFSpecial_Keystroke(2);",
        ),
        with(text("mask"), "K", "AFSpecial_KeystrokeEx('999-AA');"),
    ]);
    e.commit("pct", "0.256");
    assert_eq!(shown(&e, "pct"), "25,6%");
    assert!(e.commit("fecha", "3/11/2026").accepted);
    assert_eq!(shown(&e, "fecha"), "03/11/2026");
    assert!(!e.commit("larga", "3 de noviembre 2026").accepted, "«de» no es un mes");
    assert!(e.commit("larga", "3 Nov 2026").accepted);
    assert_eq!(shown(&e, "larga"), "3 noviembre 2026");
    assert!(e.commit("fecha", "03112026").accepted, "forma compacta");
    assert_eq!(shown(&e, "fecha"), "03/11/2026");
    assert!(!e.commit("fecha", "31/02/2026").accepted, "el 31 de febrero no existe");
    assert!(!e.commit("fecha", "hola").accepted);
    assert!(e.commit("hora", "9:5").accepted);
    assert_eq!(shown(&e, "hora"), "09:05");
    assert!(!e.commit("hora", "25:00").accepted);
    assert!(e.commit("cp", "28001").accepted);
    assert!(!e.commit("cp", "2800").accepted);
    assert!(e.commit("tel", "6125551234").accepted);
    assert_eq!(shown(&e, "tel"), "(612) 555-1234");
    assert!(e.commit("mask", "123-AB").accepted);
    assert!(!e.commit("mask", "123-A1").accepted);
    assert!(!e.commit("mask", "123").accepted, "incompleta");
    // Mensaje de fecha con el nombre del campo y el formato.
    let out = e.commit("fecha", "no");
    assert!(
        matches!(&out.messages[0], Message::Alert { text, .. } if text.contains("fecha") && text.contains("dd/mm/yyyy"))
    );
}

#[test]
fn keystroke_filters_typing() {
    let mut e = engine(vec![
        with(text("n"), "K", "AFNumber_Keystroke(2, 0, 0, 0, '', false);"),
        with(text("m"), "K", "AFSpecial_KeystrokeEx('999-AA');"),
        with(
            text("mayus"),
            "K",
            "if (!event.willCommit) event.change = event.change.toUpperCase();",
        ),
    ]);
    assert!(e.keystroke("n", "7", "12", (2, 2)).accepted);
    assert!(!e.keystroke("n", "x", "12", (2, 2)).accepted);
    assert!(e.keystroke("m", "1", "", (0, 0)).accepted);
    assert!(
        !e.keystroke("m", "A", "", (0, 0)).accepted,
        "la primera posición es una cifra"
    );
    assert!(e.keystroke("m", "A", "123-", (4, 4)).accepted);
    let out = e.keystroke("mayus", "abc", "", (0, 0));
    assert!(out.accepted);
    assert_eq!(out.text.as_deref(), Some("ABC"));
    // Un campo sin tecleo lo acepta tal cual.
    let mut e = engine(vec![text("t")]);
    let out = e.keystroke("t", "x", "", (0, 0));
    assert!(out.accepted && out.text.as_deref() == Some("x"));
}

#[test]
fn event_object_and_field_properties() {
    let mut e = engine(vec![
        with(
            text("origen"),
            "V",
            "this.getField('destino').value = event.value + '!'; this.getField('destino').readonly = true; this.getField('destino').fillColor = color.yellow; event.target.textSize = 14;",
        ),
        text("destino"),
    ]);
    let out = e.commit("origen", "hola");
    assert!(out.accepted);
    assert_eq!(value(&e, "destino"), "hola!");
    let f = e.form();
    let d = f.field("destino").unwrap();
    assert!(d.has_flag(pliego_formjs::flags::READ_ONLY));
    assert_eq!(d.fill_color, pliego_formjs::Color::Cmyk(0.0, 0.0, 1.0, 0.0));
    assert_eq!(f.field("origen").unwrap().text_size, 14.0);
    drop(f);
    assert!(out.changes.contains(&Change::Prop {
        field: "destino".into(),
        prop: "readonly".into()
    }));
    assert!(out.changed_values().contains(&"destino"));
}

#[test]
fn hide_and_show_fields() {
    let mut e = engine(vec![
        with(
            text("tiene"),
            "V",
            "this.getField('detalle').display = (event.value == 'si') ? display.visible : display.hidden;",
        ),
        text("detalle"),
    ]);
    e.commit("tiene", "no");
    assert_eq!(e.form().field("detalle").unwrap().display, 1);
    e.commit("tiene", "si");
    assert_eq!(e.form().field("detalle").unwrap().display, 0);
    e.eval("this.getField('detalle').hidden = true").unwrap();
    assert_eq!(e.form().field("detalle").unwrap().display, 1);
    assert_eq!(e.eval("this.getField('detalle').hidden").unwrap(), "true");
}

#[test]
fn checkboxes_radios_and_choices() {
    let mut chk = Field::new("acepto", FieldKind::Checkbox);
    chk.export_values = vec!["Si".into()];
    chk.value = "Off".into();
    let mut radio = Field::new("pago", FieldKind::Radio);
    radio.export_values = vec!["tarjeta".into(), "efectivo".into()];
    radio.value = "Off".into();
    let mut combo = Field::new("pais", FieldKind::Combo);
    combo.options = vec![("ES".into(), "España".into()), ("FR".into(), "Francia".into())];
    let mut e = engine(vec![
        chk,
        radio,
        combo,
        with(
            text("t"),
            "C",
            "event.value = this.getField('acepto').value + '/' + this.getField('pago').value;",
        ),
    ]);
    assert!(e.commit("acepto", "Si").accepted);
    assert_eq!(value(&e, "t"), "Si/Off");
    assert!(!e.commit("acepto", "Quizá").accepted, "estado que la casilla no tiene");
    assert!(e.commit("pago", "efectivo").accepted);
    assert_eq!(value(&e, "t"), "Si/efectivo");
    assert_eq!(
        e.eval("this.getField('pago').isBoxChecked(1) + ',' + this.getField('pago').isBoxChecked(0)")
            .unwrap(),
        "true,false"
    );
    e.eval("this.getField('acepto').checkThisBox(0, false)").unwrap();
    assert_eq!(value(&e, "acepto"), "Off");
    // Lista de opciones.
    assert_eq!(e.eval("this.getField('pais').numItems").unwrap(), "2");
    e.eval("var p = this.getField('pais'); p.insertItemAt('Italia', 'IT'); p.deleteItemAt(0);")
        .unwrap();
    assert_eq!(
        e.eval("this.getField('pais').getItemAt(0, false) + this.getField('pais').getItemAt(1, true)")
            .unwrap(),
        "FranciaIT"
    );
    e.eval("this.getField('pais').setItems([['Uno', '1'], 'Dos']);")
        .unwrap();
    assert_eq!(
        e.form().field("pais").unwrap().options,
        vec![("1".to_string(), "Uno".to_string()), ("Dos".into(), "Dos".into())]
    );
    assert!(e.commit("pais", "1").accepted);
    assert_eq!(e.eval("this.getField('pais').currentValueIndices").unwrap(), "0");
}

#[test]
fn reset_form_and_calculate_now() {
    let mut a = text("a");
    a.default_value = "7".into();
    a.value = "99".into();
    let mut e = engine(vec![
        a,
        with(text("b"), "C", "event.value = Number(this.getField('a').value) + 1;"),
    ]);
    e.eval("this.resetForm()").unwrap();
    assert_eq!(value(&e, "a"), "7");
    let out = e.take_outcome();
    assert_eq!(value(&e, "b"), "8");
    assert!(out.changed_values().contains(&"b"));
    e.eval("this.getField('a').value = 10;").unwrap();
    e.take_outcome();
    assert_eq!(value(&e, "b"), "11");
}

#[test]
fn buttons_run_their_actions() {
    let mut b = Field::new("limpiar", FieldKind::Button);
    b.scripts.insert(
        "U".into(),
        "this.getField('t').value = ''; app.alert('Hecho', 3); console.println('ok');".into(),
    );
    let mut t = text("t");
    t.value = "algo".into();
    let mut e = engine(vec![b, t]);
    let out = e.run_trigger("limpiar", "U");
    assert_eq!(value(&e, "t"), "");
    assert!(
        out.messages
            .iter()
            .any(|m| matches!(m, Message::Alert { text, icon: 3, .. } if text == "Hecho"))
    );
    assert!(out.messages.contains(&Message::Console("ok".into())));
}

#[test]
fn dangerous_actions_are_blocked_not_executed() {
    let mut e = engine(vec![]);
    e.eval("app.launchURL('http://example.com'); this.submitForm('http://example.com/x'); this.mailDoc();")
        .unwrap();
    let out = e.take_outcome();
    let actions: Vec<&str> = out
        .messages
        .iter()
        .filter_map(|m| match m {
            Message::Blocked { action, .. } => Some(action.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(actions, ["launchURL", "submitForm", "mailDoc"]);
}

#[test]
fn document_level_scripts_define_functions() {
    let form = Form {
        fields: vec![
            text("x"),
            with(text("y"), "C", "event.value = doble(Number(this.getField('x').value));"),
        ],
        doc_scripts: vec![
            ("util".into(), "function doble(n) { return n * 2; }".into()),
            ("roto".into(), "function (".into()),
        ],
        ..Default::default()
    };
    let mut e = Engine::new(form, cfg());
    assert_eq!(e.load_errors().len(), 1, "el script roto se anota y no impide el resto");
    assert_eq!(e.load_errors()[0].trigger, "doc:roto");
    e.commit("x", "4");
    assert_eq!(value(&e, "y"), "8");
}

#[test]
fn errors_in_scripts_are_reported_and_do_not_block() {
    let mut e = engine(vec![with(text("a"), "V", "this.getField('no existe').value = 1;")]);
    let out = e.commit("a", "x");
    assert!(out.accepted, "un error de script no rechaza el valor");
    assert_eq!(out.errors.len(), 1);
    assert_eq!(out.errors[0].field.as_deref(), Some("a"));
    assert_eq!(value(&e, "a"), "x");
}

#[test]
fn util_functions() {
    let mut e = engine(vec![]);
    assert_eq!(e.eval("util.printf('%,2.2f', 1234.5)").unwrap(), "1.234,50");
    assert_eq!(
        e.eval("util.printf('%d-%s-%05.1f', 3.9, 'x', 2.25)").unwrap(),
        "3-x-002.3"
    );
    assert_eq!(
        e.eval("util.printd('dd/mm/yyyy', new Date(2026, 10, 5))").unwrap(),
        "05/11/2026"
    );
    assert_eq!(
        e.eval("util.printd('dddd d mmmm', new Date(2026, 10, 5))").unwrap(),
        "jueves 5 noviembre"
    );
    assert_eq!(
        e.eval("util.printd(1, new Date(2026, 10, 5, 8, 9, 10))").unwrap(),
        "2026.11.05 08:09:10"
    );
    assert_eq!(
        e.eval("util.scand('dd/mm/yyyy', '05/11/2026').getMonth()").unwrap(),
        "10"
    );
    assert_eq!(e.eval("util.scand('dd/mm/yyyy', 'xx')").unwrap(), "null");
    assert_eq!(
        e.eval("util.printx('(999) 999-9999', '6125551234')").unwrap(),
        "(612) 555-1234"
    );
    assert_eq!(e.eval("AFMakeNumber('1.234,56')").unwrap(), "1234.56");
    assert_eq!(e.eval("AFExtractNums('a 12 b 3,5').join('|')").unwrap(), "12|3.5");
    assert_eq!(
        e.eval("app.language + app.viewerType + this.numPages + this.numFields")
            .unwrap(),
        "ESPReader30"
    );
    assert_eq!(e.eval("this.fileName").unwrap(), "formulario.pdf");
    assert_eq!(
        e.eval("color.equal(color.convert(color.red, 'CMYK'), color.red)")
            .unwrap(),
        "true"
    );
}

#[test]
fn english_messages_and_names() {
    let mut e = Engine::new(
        Form {
            fields: vec![with(text("d"), "K", "AFDate_KeystrokeEx('mmm d, yyyy');")],
            ..Default::default()
        },
        Config {
            language: Language::En,
            now_ms: Some(NOW),
            ..Default::default()
        },
    );
    assert!(e.commit("d", "Nov 3, 2026").accepted);
    let out = e.commit("d", "never");
    assert!(matches!(&out.messages[0], Message::Alert { text, .. } if text.starts_with("Invalid date/time")));
    assert_eq!(
        e.eval("util.printd('dddd mmmm', new Date(2026, 10, 5))").unwrap(),
        "Thursday November"
    );
}

#[test]
fn field_tree_parent_nodes_and_get_array() {
    let mut e = engine(vec![text("fila.a"), text("fila.b"), text("otro")]);
    e.eval("this.getField('fila').value = '9'").unwrap();
    assert_eq!(value(&e, "fila.a"), "9");
    assert_eq!(value(&e, "fila.b"), "9");
    assert_eq!(e.eval("this.getField('fila').getArray().length").unwrap(), "2");
    assert_eq!(e.eval("this.getField('inexistente')").unwrap(), "null");
    assert_eq!(e.eval("this.getNthFieldName(2)").unwrap(), "otro");
    assert_eq!(
        e.eval("this.getField('otro').type + this.getField('otro').name")
            .unwrap(),
        "textotro"
    );
}

#[test]
fn numeric_fields_return_numbers() {
    let mut e = engine(vec![
        with(text("n"), "F", "AFNumber_Format(2, 0, 0, 0, '', false);"),
        text("s"),
    ]);
    e.commit("n", "5");
    e.commit("s", "5");
    assert_eq!(
        e.eval("typeof this.getField('n').value + typeof this.getField('s').value")
            .unwrap(),
        "numberstring"
    );
    assert_eq!(e.eval("this.getField('n').value + 1").unwrap(), "6");
}

#[test]
fn format_all_prepares_display_text_on_load() {
    let mut f = with(text("n"), "F", "AFNumber_Format(2, 2, 0, 0, '', false);");
    f.value = "1234.5".into();
    let mut e = engine(vec![f, text("sin")]);
    let out = e.format_all();
    assert_eq!(shown(&e, "n"), "1.234,50");
    assert_eq!(out.display_of("n"), Some(&Some("1.234,50".to_string())));
}

#[test]
fn set_action_stores_scripts() {
    let mut e = engine(vec![text("a")]);
    e.eval("this.getField('a').setAction('Validate', 'event.rc = false;')")
        .unwrap();
    let out = e.take_outcome();
    assert!(out.changes.contains(&Change::Script {
        field: "a".into(),
        trigger: "V".into()
    }));
    assert!(!e.commit("a", "x").accepted, "la acción nueva se aplica");
}

#[test]
fn document_scripts_run_without_a_target_field() {
    let mut e = engine(vec![text("log")]);
    let out = e.run_document_script(
        "this.getField('log').value = event.type + '/' + event.name + '/' + (event.target === null);",
        "WillSave",
    );
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(value(&e, "log"), "Doc/WillSave/true");
    assert!(out.changed_values().contains(&"log"));
}
