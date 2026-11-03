use mupdf::pdf::PdfDocument;
use mupdf::{Colorspace, Matrix, Size};
use pliego_annot::*;

fn blank() -> PdfDocument {
    let mut doc = PdfDocument::new();
    doc.new_page(Size::A4).unwrap();
    doc.new_page(Size::A4).unwrap();
    doc
}

fn reopen(doc: &PdfDocument) -> PdfDocument {
    let mut buf = Vec::new();
    doc.write_to(&mut buf).unwrap();
    PdfDocument::from_bytes(&buf).unwrap()
}

/// Cuenta píxeles que no son blancos en una región de la página (coordenadas de página).
fn ink_in(doc: &PdfDocument, page: i32, r: [i32; 4]) -> usize {
    let p = doc.load_page(page).unwrap();
    let pm = p
        .to_pixmap(&Matrix::IDENTITY, &Colorspace::device_rgb(), false, true)
        .unwrap();
    let n = pm.n() as usize;
    let w = pm.width() as usize;
    let s = pm.samples();
    let mut c = 0;
    for y in r[1]..r[3] {
        for x in r[0]..r[2] {
            let i = (y as usize * w + x as usize) * n;
            if s[i] < 250 || s[i + 1] < 250 || s[i + 2] < 250 {
                c += 1;
            }
        }
    }
    c
}

#[test]
fn preset_and_text_stamps_are_visible_and_roundtrip() {
    let mut doc = blank();
    let rect = [50.0, 50.0, 250.0, 110.0];
    add(
        &mut doc,
        &AnnotSpec::new(
            0,
            Geometry::Stamp {
                rect,
                source: StampSource::Preset {
                    preset: StampPreset::Confidencial,
                },
            },
        ),
    )
    .unwrap();
    add(
        &mut doc,
        &AnnotSpec::new(
            0,
            Geometry::Stamp {
                rect: [50.0, 200.0, 300.0, 260.0],
                source: StampSource::Text {
                    text: "Revisión (año) \\ ok".into(),
                    color: [0.0, 0.0, 1.0],
                },
            },
        ),
    )
    .unwrap();
    let doc = reopen(&doc);
    assert!(ink_in(&doc, 0, [50, 50, 250, 110]) > 200, "el sello no se ve");
    assert!(ink_in(&doc, 0, [50, 200, 300, 260]) > 100);
    assert_eq!(ink_in(&doc, 1, [0, 0, 595, 842]), 0);
    let mut doc = doc;
    let l = list(&mut doc, None).unwrap();
    assert_eq!(l.iter().filter(|i| i.kind == AnnotKind::Stamp).count(), 2);
}

#[test]
fn image_stamp_keeps_aspect_ratio() {
    let dir = std::env::temp_dir().join(format!("pliego-annot-img-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("firma.pnm");
    // 4x2 píxeles negros (PNM binario).
    let mut bytes = b"P6\n4 2\n255\n".to_vec();
    bytes.extend(std::iter::repeat_n(0u8, 4 * 2 * 3));
    std::fs::write(&img, bytes).unwrap();
    let mut doc = blank();
    add(
        &mut doc,
        &AnnotSpec::new(
            0,
            Geometry::Stamp {
                rect: [100.0, 100.0, 200.0, 200.0],
                source: StampSource::Image {
                    path: img.to_str().unwrap().into(),
                },
            },
        ),
    )
    .unwrap();
    let doc = reopen(&doc);
    // Ocupa 100x50 centrado en el recuadro: arriba y abajo queda en blanco.
    assert!(ink_in(&doc, 0, [100, 125, 200, 175]) > 4000);
    assert_eq!(ink_in(&doc, 0, [100, 100, 200, 120]), 0);
    assert_eq!(ink_in(&doc, 0, [100, 180, 200, 200]), 0);
    std::fs::remove_dir_all(dir).ok();
    assert!(
        add(
            &mut blank(),
            &AnnotSpec::new(
                0,
                Geometry::Stamp {
                    rect: [0.0, 0.0, 50.0, 50.0],
                    source: StampSource::Image {
                        path: "/no/existe.png".into()
                    }
                }
            )
        )
        .is_err()
    );
}

#[test]
fn links_roundtrip() {
    let mut doc = blank();
    add(
        &mut doc,
        &AnnotSpec::new(
            0,
            Geometry::Link {
                rect: [10.0, 10.0, 100.0, 30.0],
                target: LinkTarget::Uri {
                    uri: "https://ejemplo.es/ñ".into(),
                },
            },
        ),
    )
    .unwrap();
    add(
        &mut doc,
        &AnnotSpec::new(
            0,
            Geometry::Link {
                rect: [10.0, 40.0, 100.0, 60.0],
                target: LinkTarget::Page { page: 1 },
            },
        ),
    )
    .unwrap();
    assert!(
        add(
            &mut doc,
            &AnnotSpec::new(
                0,
                Geometry::Link {
                    rect: [0.0, 0.0, 5.0, 5.0],
                    target: LinkTarget::Page { page: 7 }
                }
            )
        )
        .is_err()
    );
    let mut doc = reopen(&doc);
    let l: Vec<_> = list(&mut doc, None)
        .unwrap()
        .into_iter()
        .filter(|i| i.kind == AnnotKind::Link)
        .collect();
    let mut targets: Vec<_> = l.iter().filter_map(|i| i.link.clone()).collect();
    targets.sort();
    assert_eq!(targets, ["#2", "https://ejemplo.es/ñ"]);
}

#[test]
fn attachment_embeds_file() {
    let dir = std::env::temp_dir().join(format!("pliego-annot-att-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("notas.txt");
    std::fs::write(&f, "contenido adjunto").unwrap();
    let mut doc = blank();
    add(
        &mut doc,
        &AnnotSpec::new(
            0,
            Geometry::Attachment {
                at: [30.0, 30.0],
                path: f.to_str().unwrap().into(),
            },
        ),
    )
    .unwrap();
    let doc = reopen(&doc);
    let page = doc.load_pdf_page(0).unwrap();
    let a = page
        .annotations()
        .find(|a| a.r#type().unwrap() == mupdf::pdf::PdfAnnotationType::FileAttachment)
        .expect("adjunto");
    let fs = a.object().get_dict("FS").unwrap().unwrap();
    assert_eq!(fs.get_dict("UF").unwrap().unwrap().as_string().unwrap(), "notas.txt");
    let data = fs
        .get_dict("EF")
        .unwrap()
        .unwrap()
        .get_dict("F")
        .unwrap()
        .unwrap()
        .read_stream()
        .unwrap();
    assert_eq!(data, b"contenido adjunto");
    std::fs::remove_dir_all(dir).ok();
}
