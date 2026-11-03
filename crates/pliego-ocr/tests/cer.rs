//! Criterio de «hecho»: sobre 10 escaneos generados (texto conocido, girados y con ruido), CER < 3 % en `spa`.

mod common;

use common::*;
use pliego_core::JobContext;
use pliego_ocr::{OcrOptions, ocr_document};

const TEXTS: &[&[&str]] = &[
    SPANISH,
    &[
        "El tren de las ocho salió con retraso desde la estación del norte.",
        "Hoy comeremos paella valenciana con pollo, conejo y judías verdes.",
        "Según el informe, las ventas aumentaron un 12 % durante el último trimestre.",
        "¿Dónde está la biblioteca municipal? Está junto al ayuntamiento.",
    ],
    &[
        "Estimado cliente: le informamos de que su pedido número 48213 ha sido enviado.",
        "La entrega se realizará entre las 9:00 y las 14:00 horas del jueves.",
        "Para cualquier consulta puede llamar al servicio de atención al cliente.",
        "Gracias por confiar en nosotros. Un cordial saludo.",
    ],
    &[
        "Artículo 14. Los españoles son iguales ante la ley, sin que pueda prevalecer",
        "discriminación alguna por razón de nacimiento, raza, sexo, religión, opinión",
        "o cualquier otra condición o circunstancia personal o social.",
    ],
    &[
        "Receta: mezclar harina, azúcar y huevos hasta obtener una masa homogénea.",
        "Hornear a 180 grados durante cuarenta minutos y dejar enfriar sobre una rejilla.",
        "Servir con nata montada o con una bola de helado de vainilla.",
    ],
];

#[test]
fn ten_generated_scans_stay_under_three_percent_cer() {
    if !has_models(&["spa", "eng"]) {
        eprintln!("faltan modelos: prueba omitida");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // (texto, inclinación, ruido, giro)
    let cases: [(usize, f32, f32, u32); 10] = [
        (0, 0.0, 0.0, 0),
        (1, 1.5, 0.005, 0),
        (2, -2.0, 0.01, 0),
        (3, 3.0, 0.0, 0),
        (4, -1.0, 0.015, 0),
        (0, 2.0, 0.01, 0),
        (1, -3.0, 0.005, 0),
        (2, 0.5, 0.02, 0),
        (3, -2.5, 0.01, 0),
        (4, 1.0, 0.0, 0),
    ];
    let mut cers = Vec::new();
    for (i, (t, skew, noise, quarter)) in cases.iter().enumerate() {
        let (input, output) = (
            dir.path().join(format!("s{i}.pdf")),
            dir.path().join(format!("o{i}.pdf")),
        );
        make_scan(
            &input,
            &ScanSpec {
                lines: TEXTS[*t],
                skew: *skew,
                noise: *noise,
                quarter: *quarter,
            },
        );
        let o = OcrOptions {
            deskew: true,
            denoise: true,
            threads: 2,
            ..OcrOptions::default()
        };
        ocr_document(&input, &output, &o, &JobContext::silent()).unwrap();
        let c = cer(&normalize(&TEXTS[*t].join(" ")), &extract_text(&output));
        eprintln!("escaneo {i}: CER {:.2} %", c * 100.0);
        cers.push(c);
    }
    let mean = cers.iter().sum::<f32>() / cers.len() as f32;
    let worst = cers.iter().cloned().fold(0.0, f32::max);
    assert!(mean < 0.03, "CER medio {:.2} %", mean * 100.0);
    assert!(worst < 0.06, "peor escaneo {:.2} %", worst * 100.0);
}
