//! Planchas CMYK de imágenes, degradados, patrones e imágenes en línea (PDF construidos a mano).
//!
//! Los valores esperados salen de la definición de cada color (no de otro motor): una página entera pintada con
//! CMYK `(0.8, 0, 0, 0.2)` tiene cobertura 0,8 en cian y 0,2 en negro.

use pliego_print::{PdfDocument, Separations, separations};

/// Monta un PDF de una página 100×100 con los objetos dados (el 1 es el catálogo, el 2 las páginas, el 3 la página
/// y el 4 su contenido; los demás son libres y empiezan en el 5).
fn build(page_extra: &str, content: &str, objects: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut all: Vec<(u32, Vec<u8>)> = vec![
        (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
        (
            3,
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R {page_extra} >>")
                .into_bytes(),
        ),
        (
            4,
            [
                format!("<< /Length {} >>\nstream\n", content.len()).as_bytes(),
                content.as_bytes(),
                b"\nendstream",
            ]
            .concat(),
        ),
    ];
    all.extend(objects.iter().cloned());
    all.sort_by_key(|(n, _)| *n);
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (n, body) in &all {
        offsets.push((*n, out.len()));
        out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let size = all.last().map_or(1, |(n, _)| n + 1);
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
    for n in 1..size {
        let off = offsets.iter().find(|(m, _)| *m == n).map_or(0, |(_, o)| *o);
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    out
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    [
        format!("<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes(),
        data,
        b"\nendstream",
    ]
    .concat()
}

fn plates(bytes: &[u8]) -> Separations {
    let doc = PdfDocument::from_bytes(bytes).unwrap();
    separations(&doc, 0, 36.0).unwrap()
}

/// Cobertura media de las cuatro planchas (C, M, Y, K).
fn coverage(bytes: &[u8]) -> [f32; 4] {
    let s = plates(bytes);
    let mut out = [0.0; 4];
    for (o, p) in out.iter_mut().zip(&s.plates) {
        *o = p.coverage();
    }
    out
}

fn assert_near(got: [f32; 4], want: [f32; 4], what: &str) {
    for i in 0..4 {
        assert!(
            (got[i] - want[i]).abs() < 0.03,
            "{what}: plancha {i} = {:.3} (esperado {:.3}); todas: {got:?}",
            got[i],
            want[i]
        );
    }
}

const FULL: &str = "q 100 0 0 100 0 0 cm /Im1 Do Q";

fn image_page(dict: &str, data: &[u8], page_extra_cs: &str) -> Vec<u8> {
    build(
        &format!("/Resources << /XObject << /Im1 5 0 R >> {page_extra_cs} >>"),
        FULL,
        &[(5, stream(&format!("/Type /XObject /Subtype /Image {dict}"), data))],
    )
}

#[test]
fn cmyk_image_goes_to_its_own_plates() {
    // Mitad cian pura, mitad magenta pura.
    let pdf = image_page(
        "/Width 2 /Height 1 /ColorSpace /DeviceCMYK /BitsPerComponent 8 /Filter /ASCIIHexDecode",
        b"FF000000 00FF0000>",
        "",
    );
    assert_near(coverage(&pdf), [0.5, 0.5, 0.0, 0.0], "imagen CMYK");
}

#[test]
fn rgb_and_gray_images_use_the_simple_conversion() {
    // Rojo puro = magenta + amarillo.
    let red = image_page(
        "/Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /ASCIIHexDecode",
        b"FF0000>",
        "",
    );
    assert_near(coverage(&red), [0.0, 1.0, 1.0, 0.0], "imagen RGB roja");
    // Gris al 50 % = 50 % de negro (127 → 0,502 de tinta).
    let gray = image_page(
        "/Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /ASCIIHexDecode",
        b"80>",
        "",
    );
    assert_near(coverage(&gray), [0.0, 0.0, 0.0, 0.498], "imagen gris");
}

#[test]
fn decode_array_and_indexed_images_are_honoured() {
    // /Decode [1 0 ...] invierte: 00 000000 → todo a tinta completa en C y K.
    let inverted = image_page(
        "/Width 1 /Height 1 /ColorSpace /DeviceCMYK /BitsPerComponent 8 /Decode [1 0 1 0 1 0 1 0] /Filter /ASCIIHexDecode",
        b"00FFFF00>",
        "",
    );
    assert_near(coverage(&inverted), [1.0, 0.0, 0.0, 1.0], "/Decode invertido");
    // Indexed sobre CMYK: el índice 1 es amarillo puro.
    let indexed = image_page(
        "/Width 1 /Height 1 /ColorSpace [/Indexed /DeviceCMYK 1 <00000000 0000FF00>] /BitsPerComponent 8 /Filter /ASCIIHexDecode",
        b"01>",
        "",
    );
    assert_near(coverage(&indexed), [0.0, 0.0, 1.0, 0.0], "Indexed CMYK");
}

#[test]
fn image_inside_a_form_is_separated() {
    let pdf = build(
        "/Resources << /XObject << /Fm1 6 0 R >> >>",
        "/Fm1 Do",
        &[
            (
                5,
                stream(
                    "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceCMYK /BitsPerComponent 8 /Filter /ASCIIHexDecode",
                    b"00006600>",
                ),
            ),
            (
                6,
                stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /Im1 5 0 R >> >>",
                    FULL.as_bytes(),
                ),
            ),
        ],
    );
    assert_near(coverage(&pdf), [0.0, 0.0, 0.4, 0.0], "imagen dentro de un formulario");
}

#[test]
fn inline_image_is_separated() {
    let content = "q 100 0 0 100 0 0 cm BI /W 1 /H 1 /CS /CMYK /BPC 8 /F /AHx ID 00FF0000> EI Q";
    let pdf = build("", content, &[]);
    assert_near(coverage(&pdf), [0.0, 1.0, 0.0, 0.0], "imagen en línea");
}

fn cmyk_jpeg(color: [u8; 4], kind: jpeg_encoder::ColorType) -> Vec<u8> {
    let mut px = Vec::new();
    for _ in 0..16 * 16 {
        px.extend_from_slice(&color);
    }
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, 100)
        .encode(&px, 16, 16, kind)
        .unwrap();
    out
}

#[test]
fn cmyk_and_ycck_jpegs_are_separated() {
    // C 80 %, K 20 %. El codificador los guarda invertidos (como Photoshop), y el PDF lo corrige con /Decode.
    for kind in [jpeg_encoder::ColorType::Cmyk, jpeg_encoder::ColorType::CmykAsYcck] {
        let jpeg = cmyk_jpeg([204, 0, 0, 51], kind);
        let pdf = image_page(
            "/Width 16 /Height 16 /ColorSpace /DeviceCMYK /BitsPerComponent 8 /Decode [1 0 1 0 1 0 1 0] /Filter /DCTDecode",
            &jpeg,
            "",
        );
        assert_near(coverage(&pdf), [0.8, 0.0, 0.0, 0.2], &format!("JPEG {kind:?}"));
    }
}

#[test]
fn unsupported_image_is_left_alone() {
    // Separation: no se decodifica; no debe fallar y el documento sigue produciendo cuatro planchas.
    let pdf = image_page(
        "/Width 1 /Height 1 /ColorSpace [/Separation /Naranja /DeviceCMYK << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 0.5 1 0] /N 1 >>] /BitsPerComponent 8 /Filter /ASCIIHexDecode",
        b"FF>",
        "",
    );
    assert_eq!(plates(&pdf).plates.len(), 4);
}

fn shading_page(shading: &str, extra: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut objs = vec![(5, shading.as_bytes().to_vec())];
    objs.extend(extra.iter().cloned());
    build("/Resources << /Shading << /Sh1 5 0 R >> >>", "/Sh1 sh", &objs)
}

#[test]
fn axial_shading_with_exponential_function() {
    // De blanco a cian puro de izquierda a derecha: cobertura media 0,5 en cian.
    let pdf = shading_page(
        "<< /ShadingType 2 /ColorSpace /DeviceCMYK /Coords [0 0 100 0] /Extend [true true] \
         /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 0 0 0] /N 1 >> >>",
        &[],
    );
    assert_near(coverage(&pdf), [0.5, 0.0, 0.0, 0.0], "degradado exponencial");
}

#[test]
fn radial_shading_with_stitching_function() {
    // Dos mitades: cian→cian (0,4) y magenta→magenta (0,6) con tinta constante en cada tramo; los coeficientes
    // dan 0,5·0,4 + 0,5·0,6... lo importante es que cian y magenta ocupan cada uno la mitad de la página.
    let pdf = shading_page(
        "<< /ShadingType 2 /ColorSpace /DeviceCMYK /Coords [0 0 100 0] /Extend [true true] \
         /Function << /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] /Functions [ \
           << /FunctionType 2 /Domain [0 1] /C0 [1 0 0 0] /C1 [1 0 0 0] /N 1 >> \
           << /FunctionType 2 /Domain [0 1] /C0 [0 1 0 0] /C1 [0 1 0 0] /N 1 >> ] >> >>",
        &[],
    );
    assert_near(coverage(&pdf), [0.5, 0.5, 0.0, 0.0], "función a trozos");
}

#[test]
fn sampled_function_shading() {
    // Muestras de 8 bits: del amarillo al negro (2 muestras de 4 componentes).
    let pdf = shading_page(
        "<< /ShadingType 2 /ColorSpace /DeviceCMYK /Coords [0 0 100 0] /Extend [true true] /Function 6 0 R >>",
        &[(
            6,
            stream(
                "/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1 0 1] /Size [2] /BitsPerSample 8 /Filter /ASCIIHexDecode",
                b"00 00 FF 00  00 00 00 FF>",
            ),
        )],
    );
    // Yo (amarillo) baja de 1 a 0; K sube de 0 a 1: ambos con media 0,5.
    assert_near(coverage(&pdf), [0.0, 0.0, 0.5, 0.5], "función muestreada");
}

#[test]
fn rgb_and_icc_shadings_are_converted() {
    let pdf = shading_page(
        "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 100 0] /Extend [true true] \
         /Function << /FunctionType 2 /Domain [0 1] /C0 [1 1 1] /C1 [1 1 1] /N 1 >> >>",
        &[],
    );
    // Blanco: sin tinta.
    assert_near(coverage(&pdf), [0.0, 0.0, 0.0, 0.0], "degradado RGB blanco");
    let pdf = shading_page(
        "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 100 0] /Extend [true true] \
         /Function << /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 1 >> >>",
        &[],
    );
    // Del blanco al negro: el negro sube de 0 a 1.
    assert_near(coverage(&pdf), [0.0, 0.0, 0.0, 0.5], "degradado gris");
}

#[test]
fn shading_pattern_fill_is_separated() {
    let pdf = build(
        "/Resources << /ColorSpace << /Cs1 [/Pattern] >> /Pattern << /P1 5 0 R >> >>",
        "/Pattern cs /P1 scn 0 0 100 100 re f",
        &[(
            5,
            b"<< /PatternType 2 /Shading << /ShadingType 2 /ColorSpace /DeviceCMYK /Coords [0 0 100 0] /Extend [true true] \
              /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 0 1 0] /N 1 >> >> >>"
                .to_vec(),
        )],
    );
    assert_near(coverage(&pdf), [0.0, 0.0, 0.5, 0.0], "patrón de degradado");
}

#[test]
fn tiling_pattern_with_an_image_is_separated() {
    let pdf = build(
        "/Resources << /Pattern << /P1 6 0 R >> >>",
        "/Pattern cs /P1 scn 0 0 100 100 re f",
        &[
            (
                5,
                stream(
                    "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceCMYK /BitsPerComponent 8 /Filter /ASCIIHexDecode",
                    b"0000FF00>",
                ),
            ),
            (
                6,
                stream(
                    "/PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 100 100] /XStep 100 /YStep 100 \
                     /Resources << /XObject << /Im1 5 0 R >> >>",
                    FULL.as_bytes(),
                ),
            ),
        ],
    );
    assert_near(coverage(&pdf), [0.0, 0.0, 1.0, 0.0], "patrón de mosaico con imagen");
}

#[test]
fn total_ink_counts_images() {
    // C 100 + M 100 + Y 50 + K 50 = 300 %.
    let pdf = image_page(
        "/Width 1 /Height 1 /ColorSpace /DeviceCMYK /BitsPerComponent 8 /Filter /ASCIIHexDecode",
        b"FFFF8080>",
        "",
    );
    let s = plates(&pdf);
    assert!((s.max_total_ink - 3.0).abs() < 0.05, "{}", s.max_total_ink);
}
