//! Formulario de prueba construido a mano con el modelo nativo (texto, casilla, radios, combo, lista y botón).

use super::PdfDocument as Native;
use super::object::Object;
use crate::Size;

fn add(doc: &mut Native, src: &str) -> Object {
    let o = doc.new_object_from_str(src).unwrap();
    doc.add_object(&o).unwrap()
}

fn id(o: &Object) -> i32 {
    o.as_indirect().unwrap()
}

fn form_xobject(doc: &mut Native, w: f32, h: f32, content: &str) -> Object {
    let d = doc
        .new_object_from_str(&format!("<< /Type /XObject /Subtype /Form /BBox [0 0 {w} {h}] >>"))
        .unwrap();
    doc.add_stream(content.as_bytes(), Some(&d), false).unwrap()
}

/// Una página A4 con seis campos. Devuelve el documento.
pub(super) fn form_doc() -> Native {
    let mut doc = Native::new();
    let mut page = doc.new_page(Size::A4).unwrap();
    let helv = add(
        &mut doc,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    let pid = id(&page.object());

    let text = add(
        &mut doc,
        &format!(
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (nombre) /V (inicial) /DV (inicial) /Rect [50 700 250 720] \
             /F 4 /P {pid} 0 R /DA (/Helv 12 Tf 0 g) /MK << /BC [0] /BG [1] >> >>"
        ),
    );

    let on = form_xobject(&mut doc, 20.0, 20.0, "0 g 4 4 12 12 re f");
    let off = form_xobject(&mut doc, 20.0, 20.0, "");
    let check = add(
        &mut doc,
        &format!(
            "<< /Type /Annot /Subtype /Widget /FT /Btn /T (acepto) /V /Off /AS /Off /Rect [50 650 70 670] /F 4 \
             /P {pid} 0 R /MK << /BC [0] >> /AP << /N << /Si {} 0 R /Off {} 0 R >> >> >>",
            id(&on),
            id(&off)
        ),
    );

    let r_on1 = form_xobject(&mut doc, 20.0, 20.0, "1 0 0 rg 4 4 12 12 re f");
    let r_on2 = form_xobject(&mut doc, 20.0, 20.0, "0 1 0 rg 4 4 12 12 re f");
    let r_off = form_xobject(&mut doc, 20.0, 20.0, "");
    let radio_parent = add(&mut doc, "<< /FT /Btn /Ff 32768 /T (color) /V /Off /Kids [] >>");
    let radio1 = add(
        &mut doc,
        &format!(
            "<< /Type /Annot /Subtype /Widget /Parent {} 0 R /Rect [50 600 70 620] /F 4 /P {pid} 0 R /AS /Off \
             /AP << /N << /rojo {} 0 R /Off {} 0 R >> >> >>",
            id(&radio_parent),
            id(&r_on1),
            id(&r_off)
        ),
    );
    let radio2 = add(
        &mut doc,
        &format!(
            "<< /Type /Annot /Subtype /Widget /Parent {} 0 R /Rect [80 600 100 620] /F 4 /P {pid} 0 R /AS /Off \
             /AP << /N << /verde {} 0 R /Off {} 0 R >> >> >>",
            id(&radio_parent),
            id(&r_on2),
            id(&r_off)
        ),
    );
    let mut rp = radio_parent.clone();
    let kids = doc
        .new_object_from_str(&format!("[{} 0 R {} 0 R]", id(&radio1), id(&radio2)))
        .unwrap();
    rp.dict_put("Kids", kids).unwrap();

    let combo = add(
        &mut doc,
        &format!(
            "<< /Type /Annot /Subtype /Widget /FT /Ch /Ff 131072 /T (pais) /V (ES) /Opt [(ES) (FR) (DE)] \
             /Rect [50 550 250 570] /F 4 /P {pid} 0 R /DA (/Helv 12 Tf 0 g) /MK << /BC [0] >> >>"
        ),
    );
    let list = add(
        &mut doc,
        &format!(
            "<< /Type /Annot /Subtype /Widget /FT /Ch /T (lista) /Opt [[(a) (Alfa)] [(b) (Beta)]] \
             /Rect [50 450 250 530] /F 4 /P {pid} 0 R /DA (/Helv 12 Tf 0 g) /MK << /BC [0] >> >>"
        ),
    );
    let button = add(
        &mut doc,
        &format!(
            "<< /Type /Annot /Subtype /Widget /FT /Btn /Ff 65536 /T (enviar) /Rect [50 400 150 430] /F 4 \
             /P {pid} 0 R /MK << /CA (Enviar) /BG [0.8] /BC [0] >> >>"
        ),
    );

    let ids = [&text, &check, &radio1, &radio2, &combo, &list, &button];
    let annots = format!(
        "[{}]",
        ids.iter()
            .map(|o| format!("{} 0 R", id(o)))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut p = page.object();
    p.dict_put("Annots", doc.new_object_from_str(&annots).unwrap()).unwrap();

    let fields = format!(
        "[{}]",
        [&text, &check, &radio_parent, &combo, &list, &button]
            .iter()
            .map(|o| format!("{} 0 R", id(o)))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let acro = add(
        &mut doc,
        &format!(
            "<< /Fields {fields} /DA (/Helv 0 Tf 0 g) /DR << /Font << /Helv {} 0 R >> >> >>",
            id(&helv)
        ),
    );
    doc.catalog().unwrap().dict_put("AcroForm", acro).unwrap();
    let _ = &mut page;
    doc
}
