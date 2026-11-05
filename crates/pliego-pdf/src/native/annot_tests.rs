//! Pruebas de las anotaciones nativas. MuPDF actúa de motor de referencia: dibuja lo que escribe `native` y
//! lee los mismos valores que la capa sobre MuPDF.

use super::PdfDocument as Native;
use super::tests::qpdf_check;
use crate::{
    AnnotationBorderStyle, AnnotationColor, AnnotationFlags, AnnotationTextAlign, Colorspace, Document,
    LineEndingStyle, Matrix, PdfAnnotationType, Point, Quad, Rect, Size, WriteOptions,
};

fn quad(x0: f32, y0: f32, x1: f32, y1: f32) -> Quad {
    Quad::new(
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x0, y1),
        Point::new(x1, y1),
    )
}

fn rgb(r: f32, g: f32, b: f32) -> AnnotationColor {
    AnnotationColor::Rgb {
        red: r,
        green: g,
        blue: b,
    }
}

/// Documento con una página A4 y la frase «Hola mundo» (Helvetica 24) arriba a la izquierda.
fn sample() -> Native {
    let mut doc = Native::new();
    let mut page = doc.new_page(Size::A4).unwrap();
    let font = doc
        .new_object_from_str("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
        .unwrap();
    let font = doc.add_object(&font).unwrap();
    let mut res = page.resources().unwrap();
    let mut fonts = doc.new_dict().unwrap();
    fonts.dict_put("F1", font).unwrap();
    res.dict_put("Font", fonts).unwrap();
    page.insert_contents(&mut doc, b"BT /F1 24 Tf 72 700 Td (Hola mundo) Tj ET", true)
        .unwrap();
    doc
}

fn render(bytes: &[u8]) -> (Vec<u8>, usize, u32) {
    let doc = Document::from_bytes(bytes, "application/pdf").unwrap();
    let page = doc.load_page(0).unwrap();
    let pix = page
        .to_pixmap(&Matrix::IDENTITY, Colorspace::device_rgb(), false, true)
        .unwrap();
    (pix.samples().to_vec(), pix.stride(), pix.width())
}

/// Con `PLIEGO_DUMP_DIR` definido, guarda el PDF y un PNG de la página 1 para mirarlos a ojo.
fn dump_png(name: &str, bytes: &[u8]) {
    let Ok(dir) = std::env::var("PLIEGO_DUMP_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{dir}/{name}.pdf"), bytes).unwrap();
    let doc = Document::from_bytes(bytes, "application/pdf").unwrap();
    let page = doc.load_page(0).unwrap();
    let pix = page
        .to_pixmap(&Matrix::new_scale(2.0, 2.0), Colorspace::device_rgb(), false, true)
        .unwrap();
    std::fs::write(format!("{dir}/{name}.png"), pix.encode_png().unwrap()).unwrap();
}

struct Shot {
    px: Vec<u8>,
    stride: usize,
    width: u32,
}

impl Shot {
    fn new(bytes: &[u8]) -> Shot {
        let (px, stride, width) = render(bytes);
        Shot { px, stride, width }
    }
    fn at(&self, x: u32, y: u32) -> (u8, u8, u8) {
        assert!(x < self.width);
        let i = y as usize * self.stride + x as usize * 3;
        (self.px[i], self.px[i + 1], self.px[i + 2])
    }
    /// ¿Algún píxel de la región (x0..x1, y0..y1) cumple `f`?
    fn any(&self, x0: u32, y0: u32, x1: u32, y1: u32, f: impl Fn((u8, u8, u8)) -> bool) -> bool {
        (y0..y1).any(|y| (x0..x1).any(|x| f(self.at(x, y))))
    }
    fn non_white(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| self.at(x, y) != (255, 255, 255))
            .count()
    }
}

fn reddish(p: (u8, u8, u8)) -> bool {
    p.0 > 180 && p.1 < 90 && p.2 < 90
}
/// Rojo antialias (una línea de 1 pt a caballo entre dos filas de píxeles).
fn soft_red(p: (u8, u8, u8)) -> bool {
    p.0 > 200 && p.1 < 170 && p.2 < 170
}
fn greenish(p: (u8, u8, u8)) -> bool {
    p.1 > 180 && p.0 < 90 && p.2 < 90
}
fn bluish(p: (u8, u8, u8)) -> bool {
    p.2 > 180 && p.0 < 90 && p.1 < 90
}
fn yellowish(p: (u8, u8, u8)) -> bool {
    p.0 > 180 && p.1 > 180 && p.2 < 120
}

fn saved(doc: &Native) -> Vec<u8> {
    let out = doc.to_bytes(&WriteOptions::new()).unwrap();
    if let Some((code, log)) = qpdf_check(&out, None) {
        assert_eq!(code, 0, "qpdf --check:\n{log}");
    }
    out
}

#[test]
fn every_kind_is_created_and_drawn() {
    let doc = sample();
    let mut page = doc.load_pdf_page(0).unwrap();

    // Resaltado, subrayado, tachado y ondulado sobre «Hola mundo» (en espacio de página: y ≈ 120..145).
    let q = quad(72.0, 118.0, 170.0, 146.0);
    let mut hl = page.add_highlight_annotation(&[q]).unwrap();
    hl.update().unwrap();
    let mut ul = page
        .add_underline_annotation(&[quad(72.0, 218.0, 170.0, 246.0)])
        .unwrap();
    ul.update().unwrap();
    let mut so = page
        .add_strikeout_annotation(&[quad(72.0, 258.0, 170.0, 286.0)])
        .unwrap();
    so.update().unwrap();
    let mut sq = page
        .add_squiggly_annotation(&[quad(72.0, 298.0, 170.0, 326.0)])
        .unwrap();
    sq.update().unwrap();

    // Cuadrado con relleno, círculo, línea con flecha, polígono, polilínea y lápiz.
    let mut square = page
        .add_square_annotation(Rect::new(200.0, 100.0, 280.0, 160.0))
        .unwrap();
    square.set_color(rgb(1.0, 0.0, 0.0)).unwrap();
    square.set_interior_color(rgb(0.0, 1.0, 0.0)).unwrap();
    square.set_border_width(4.0).unwrap();
    square.update().unwrap();
    let mut circle = page
        .add_circle_annotation(Rect::new(300.0, 100.0, 380.0, 160.0))
        .unwrap();
    circle.update().unwrap();
    let mut line = page
        .add_line_annotation(Point::new(200.0, 200.0), Point::new(300.0, 260.0))
        .unwrap();
    line.set_line_ending_styles(LineEndingStyle::None, LineEndingStyle::ClosedArrow)
        .unwrap();
    line.update().unwrap();
    let mut poly = page
        .add_polygon_annotation(&[
            Point::new(320.0, 200.0),
            Point::new(380.0, 200.0),
            Point::new(350.0, 260.0),
        ])
        .unwrap();
    poly.update().unwrap();
    let mut pl = page
        .add_polyline_annotation(&[
            Point::new(400.0, 200.0),
            Point::new(460.0, 200.0),
            Point::new(430.0, 260.0),
        ])
        .unwrap();
    pl.update().unwrap();
    let mut ink = page
        .add_ink_annotation(&[vec![
            Point::new(480.0, 200.0),
            Point::new(540.0, 230.0),
            Point::new(500.0, 260.0),
        ]])
        .unwrap();
    ink.update().unwrap();

    // Nota, texto libre, sello y cursor.
    let mut note = page
        .add_text_annotation(Rect::new(72.0, 400.0, 92.0, 420.0), "una nota")
        .unwrap();
    note.set_icon_name("Comment").unwrap();
    note.update().unwrap();
    let mut free = page
        .add_free_text_annotation(
            Rect::new(120.0, 400.0, 320.0, 460.0),
            "Texto libre con varias palabras para que se reparta en líneas",
        )
        .unwrap();
    free.set_default_appearance("Helv", 12.0, Some(rgb(0.0, 0.0, 1.0)))
        .unwrap();
    free.update().unwrap();
    let mut stamp = page
        .add_stamp_annotation(Rect::new(340.0, 400.0, 460.0, 440.0), "Approved")
        .unwrap();
    stamp.update().unwrap();
    let mut caret = page
        .add_caret_annotation(Rect::new(480.0, 400.0, 500.0, 414.0))
        .unwrap();
    caret.update().unwrap();

    let bytes = saved(&doc);
    dump_png("anotaciones", &bytes);
    let shot = Shot::new(&bytes);

    assert!(
        shot.any(80, 125, 160, 140, |p| p.0 > 200
            && p.1 > 200
            && p.2 < 200
            && p != (255, 255, 255)),
        "resaltado"
    );
    assert!(
        shot.non_white(72, 118, 170, 146) > 200,
        "el texto sigue visible bajo el resaltado"
    );
    assert!(shot.any(72, 222, 170, 248, greenish), "subrayado");
    assert!(shot.any(72, 258, 170, 290, soft_red), "tachado");
    assert!(
        shot.any(72, 298, 170, 332, |p| p.0 > 150 && p.2 > 150 && p.1 < 90),
        "ondulado"
    );
    assert!(shot.any(205, 105, 275, 155, greenish), "relleno del cuadrado");
    assert!(shot.any(198, 100, 206, 160, reddish), "borde del cuadrado");
    assert!(shot.any(330, 98, 350, 106, reddish), "borde del círculo");
    assert!(shot.any(240, 220, 260, 240, reddish), "línea");
    assert!(shot.any(330, 196, 370, 206, soft_red), "polígono");
    assert!(shot.any(400, 196, 460, 204, soft_red), "polilínea");
    assert!(shot.any(480, 198, 545, 262, reddish), "lápiz");
    assert!(shot.any(72, 400, 92, 420, yellowish), "nota");
    assert!(shot.any(120, 400, 320, 460, bluish), "texto libre en azul");
    assert!(shot.any(340, 400, 460, 440, reddish), "sello");
    assert!(shot.any(480, 400, 500, 414, bluish), "cursor");
}

#[test]
fn getters_return_what_the_setters_wrote() {
    let doc = sample();
    let mut page = doc.load_pdf_page(0).unwrap();
    let mut a = page
        .add_square_annotation(Rect::new(100.0, 100.0, 200.0, 160.0))
        .unwrap();
    assert_eq!(a.r#type().unwrap(), PdfAnnotationType::Square);
    assert_eq!(a.rect().unwrap(), Rect::new(100.0, 100.0, 200.0, 160.0));
    assert_eq!(a.flags().unwrap(), AnnotationFlags::IS_PRINT);
    assert_eq!(a.border_width().unwrap(), 1.0);
    assert_eq!(a.border_style().unwrap(), AnnotationBorderStyle::Solid);
    assert_eq!(a.color().unwrap(), Some(rgb(1.0, 0.0, 0.0)));
    assert_eq!(a.interior_color().unwrap(), None);
    assert_eq!(a.opacity().unwrap(), 1.0);
    a.set_border_style(AnnotationBorderStyle::Dashed).unwrap();
    a.set_border_dash_pattern(&[3.0, 2.0]).unwrap();
    a.set_border_width(3.0).unwrap();
    a.set_opacity(0.5).unwrap();
    a.set_author("Ana").unwrap();
    a.set_contents("ñandú → 日本").unwrap();
    a.set_flags(AnnotationFlags::IS_PRINT | AnnotationFlags::IS_LOCKED)
        .unwrap();
    a.set_interior_color(AnnotationColor::Gray(0.5)).unwrap();
    assert!(a.update().unwrap());
    // Tras regenerar, el rectángulo crece lo que pide el grosor (ceil(3/2) = 2 por lado), con el trazo dentro.
    assert_eq!(a.rect().unwrap(), Rect::new(98.0, 98.0, 202.0, 162.0));
    assert!(!a.update().unwrap(), "sin cambios no se regenera");
    assert_eq!(a.border_style().unwrap(), AnnotationBorderStyle::Dashed);
    assert_eq!(a.border_dash_pattern().unwrap(), vec![3.0, 2.0]);
    assert_eq!(a.author().unwrap().as_deref(), Some("Ana"));
    assert_eq!(a.contents().unwrap().as_deref(), Some("ñandú → 日本"));
    assert_eq!(a.interior_color().unwrap(), Some(AnnotationColor::Gray(0.5)));

    // Persisten al guardar y reabrir.
    let bytes = saved(&doc);
    let again = Native::from_bytes(&bytes).unwrap();
    let p2 = again.load_pdf_page(0).unwrap();
    let b = p2.annotations().next().unwrap();
    assert_eq!(b.contents().unwrap().as_deref(), Some("ñandú → 日本"));
    assert_eq!(b.flags().unwrap().bits(), 4 | 128);
    assert_eq!(b.opacity().unwrap(), 0.5);
    assert_eq!(b.border_width().unwrap(), 3.0);
}

#[test]
fn geometry_in_page_space_survives_a_rotated_page() {
    let doc = sample();
    let mut page = doc.load_pdf_page(0).unwrap();
    page.set_rotation(90).unwrap();
    let q = quad(100.0, 100.0, 200.0, 120.0);
    let mut hl = page.add_highlight_annotation(&[q]).unwrap();
    hl.update().unwrap();
    let back = hl.quad_points().unwrap();
    assert_eq!(back.len(), 1);
    for (a, b) in [
        (back[0].ul, q.ul),
        (back[0].ur, q.ur),
        (back[0].ll, q.ll),
        (back[0].lr, q.lr),
    ] {
        assert!((a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3, "{a:?} != {b:?}");
    }
    let r = hl.rect().unwrap();
    assert!(
        r.x0 <= 100.0 && r.x1 >= 200.0 && r.y0 <= 100.0 && r.y1 >= 120.0,
        "{r:?}"
    );

    let mut ln = page
        .add_line_annotation(Point::new(10.0, 300.0), Point::new(90.0, 340.0))
        .unwrap();
    assert_eq!(ln.line().unwrap(), (Point::new(10.0, 300.0), Point::new(90.0, 340.0)));
    ln.update().unwrap();

    // Dibujo: el resaltado está donde se pidió, con la página girada.
    let bytes = saved(&doc);
    let shot = Shot::new(&bytes);
    assert!(shot.any(105, 103, 195, 117, |p| p.0 > 200 && p.1 > 200 && p.2 < 160));
    assert!(!shot.any(300, 100, 400, 120, |p| p.2 < 160 && p.0 > 200));
}

#[test]
fn update_only_touches_annotations_that_changed() {
    // Una anotación de otro programa (aquí, escrita por MuPDF) se conserva al abrir y guardar.
    let mut p = crate::PdfDocument::new();
    drop(p.new_page(Size::A4).unwrap());
    let p = crate::PdfDocument::from_bytes(&p.to_bytes(&WriteOptions::new()).unwrap()).unwrap();
    {
        let mut page = p.load_pdf_page(0).unwrap();
        let mut hl = page.add_highlight_annotation(&[quad(50.0, 50.0, 150.0, 70.0)]).unwrap();
        hl.update().unwrap();
    }
    let bytes = p.to_bytes(&WriteOptions::new()).unwrap();

    let doc = Native::from_bytes(&bytes).unwrap();
    let mut page = doc.load_pdf_page(0).unwrap();
    let mut a = page.annotations().next().unwrap();
    let ap_before = a
        .object()
        .get_dict("AP")
        .unwrap()
        .unwrap()
        .get_dict("N")
        .unwrap()
        .unwrap()
        .read_stream()
        .unwrap();
    assert!(!a.update().unwrap(), "no ha cambiado: no se toca su apariencia");
    assert!(!page.update().unwrap());
    a.set_color(rgb(0.0, 0.0, 1.0)).unwrap();
    assert!(page.update().unwrap(), "cambiada: se regenera");
    let ap_after = a
        .object()
        .get_dict("AP")
        .unwrap()
        .unwrap()
        .get_dict("N")
        .unwrap()
        .unwrap()
        .read_stream()
        .unwrap();
    assert_ne!(ap_before, ap_after);
    assert!(String::from_utf8_lossy(&ap_after).contains("0 0 1 rg"));
}

#[test]
fn delete_removes_annotation_and_popup_and_links_are_not_annotations() {
    let doc = sample();
    let mut page = doc.load_pdf_page(0).unwrap();
    let mut note = page
        .add_text_annotation(Rect::new(10.0, 10.0, 30.0, 30.0), "x")
        .unwrap();
    note.update().unwrap();
    let hl = page.add_highlight_annotation(&[quad(50.0, 50.0, 150.0, 70.0)]).unwrap();
    // La nota crea además su ventana emergente, que no cuenta como anotación.
    assert_eq!(page.annotations().count(), 2);
    let annots = page.object().get_dict("Annots").unwrap().unwrap().len().unwrap();
    assert_eq!(annots, 3);
    page.delete_annotation(note).unwrap();
    assert_eq!(page.annotations().count(), 1);
    assert_eq!(page.object().get_dict("Annots").unwrap().unwrap().len().unwrap(), 1);
    drop(hl);
}

#[test]
fn links_and_widgets_are_not_annotations() {
    for f in ["organize/links.pdf", "organize/form1.pdf"] {
        let bytes = super::tests::corpus_file(f);
        let doc = Native::from_bytes(&bytes).unwrap();
        assert_eq!(doc.load_pdf_page(0).unwrap().annotations().count(), 0, "{f}");
    }
}

#[test]
fn free_text_wraps_inside_its_rectangle_and_aligns() {
    let doc = sample();
    let mut page = doc.load_pdf_page(0).unwrap();
    let mut a = page
        .add_free_text_annotation(
            Rect::new(100.0, 100.0, 200.0, 200.0),
            "uno dos tres cuatro cinco seis siete ocho nueve",
        )
        .unwrap();
    a.set_default_appearance("Helv", 10.0, None).unwrap();
    a.set_quadding(AnnotationTextAlign::Right).unwrap();
    a.update().unwrap();
    assert_eq!(a.quadding().unwrap(), AnnotationTextAlign::Right);
    let da = a.default_appearance().unwrap().unwrap();
    assert_eq!((da.font_name.as_str(), da.size), ("Helv", 10.0));
    let stream = a
        .object()
        .get_dict("AP")
        .unwrap()
        .unwrap()
        .get_dict("N")
        .unwrap()
        .unwrap()
        .read_stream()
        .unwrap();
    let text = String::from_utf8_lossy(&stream);
    assert!(text.matches("Tj").count() >= 3, "varias líneas:\n{text}");
    let bytes = saved(&doc);
    let shot = Shot::new(&bytes);
    // Alineado a la derecha: más tinta en la mitad derecha que en la izquierda.
    assert!(shot.non_white(150, 100, 200, 200) > shot.non_white(100, 100, 120, 200));
}

/// El mismo guion sobre la capa con MuPDF y sobre el modelo nativo: los getters coinciden.
#[test]
fn getters_match_the_mupdf_layer() {
    macro_rules! script {
        ($page:expr) => {{
            let page = &mut $page;
            let mut out: Vec<String> = Vec::new();
            let q = quad(100.0, 100.0, 200.0, 120.0);
            let mut hl = page.add_highlight_annotation(&[q]).unwrap();
            hl.set_contents("hola").unwrap();
            hl.set_author("yo").unwrap();
            let mut ln = page
                .add_line_annotation(Point::new(10.0, 10.0), Point::new(90.0, 40.0))
                .unwrap();
            ln.set_line_ending_styles(LineEndingStyle::None, LineEndingStyle::OpenArrow)
                .unwrap();
            let mut pg = page
                .add_polygon_annotation(&[
                    Point::new(10.0, 300.0),
                    Point::new(90.0, 300.0),
                    Point::new(50.0, 360.0),
                ])
                .unwrap();
            pg.set_border_width(2.0).unwrap();
            let ink = page
                .add_ink_annotation(&[vec![Point::new(210.0, 500.0), Point::new(290.0, 520.0)]])
                .unwrap();
            let mut sq = page
                .add_square_annotation(Rect::new(300.0, 300.0, 380.0, 360.0))
                .unwrap();
            sq.set_interior_color(AnnotationColor::Gray(0.5)).unwrap();
            sq.set_opacity(0.25).unwrap();
            sq.set_border_style(AnnotationBorderStyle::Dashed).unwrap();
            for a in [&hl, &ln, &pg, &ink, &sq] {
                out.push(format!(
                    "{:?} {:?} {:?} {:?}",
                    a.r#type().unwrap(),
                    a.flags().unwrap(),
                    a.color().unwrap(),
                    a.opacity().unwrap(),
                ));
            }
            for a in [&ln, &pg, &ink, &sq] {
                out.push(format!(
                    "{:?} {:?}",
                    a.border_width().unwrap(),
                    a.border_style().unwrap()
                ));
            }
            out.push(format!("{:?} {:?}", hl.quad_points().unwrap(), hl.contents().unwrap()));
            out.push(format!(
                "{:?} {:?}",
                ln.line().unwrap(),
                ln.line_ending_styles().unwrap()
            ));
            out.push(format!("{:?}", pg.vertices().unwrap()));
            out.push(format!("{:?}", ink.ink_list().unwrap()));
            out.push(format!("{:?}", sq.interior_color().unwrap()));
            out.push(format!("{}", page.annotations().count()));
            out
        }};
    }
    let native = sample();
    let mut np = native.load_pdf_page(0).unwrap();
    let a = script!(np);

    let mut m = crate::PdfDocument::new();
    let mut mp = m.new_page(Size::A4).unwrap();
    let b = script!(mp);
    drop(mp);
    drop(m);
    assert_eq!(a, b);
}
