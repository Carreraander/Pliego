use pliego_annot::*;
use pliego_pdf::PdfDocument;
use pliego_pdf::Size;

fn blank() -> PdfDocument {
    let mut doc = PdfDocument::new();
    doc.new_page(Size::A4).unwrap();
    doc.new_page(Size::A4).unwrap();
    doc
}

/// Guarda a memoria y reabre, como haría un ciclo guardar/abrir real.
fn reopen(doc: &PdfDocument) -> PdfDocument {
    let mut buf = Vec::new();
    doc.write_to(&mut buf).unwrap();
    PdfDocument::from_bytes(&buf).unwrap()
}

fn styled(mut s: AnnotSpec) -> AnnotSpec {
    s.style = Style {
        color: Some([1.0, 0.0, 0.0]),
        interior_color: None,
        opacity: Some(0.5),
        border_width: Some(2.0),
    };
    s.author = Some("Ana".into());
    s.subject = Some("Revisión".into());
    s.contents = Some("Hola ñandú".into());
    s
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn geometries() -> Vec<(Geometry, AnnotKind)> {
    let q = [100.0, 100.0, 200.0, 100.0, 100.0, 120.0, 200.0, 120.0];
    vec![
        (Geometry::Note { at: [50.0, 50.0] }, AnnotKind::Note),
        (
            Geometry::FreeText {
                rect: [50.0, 300.0, 250.0, 340.0],
                font_size: 12.0,
            },
            AnnotKind::FreeText,
        ),
        (Geometry::Highlight { quads: vec![q] }, AnnotKind::Highlight),
        (Geometry::Underline { quads: vec![q] }, AnnotKind::Underline),
        (Geometry::StrikeOut { quads: vec![q] }, AnnotKind::StrikeOut),
        (Geometry::Squiggly { quads: vec![q] }, AnnotKind::Squiggly),
        (
            Geometry::Line {
                start: [10.0, 400.0],
                end: [200.0, 450.0],
                start_style: LineEnd::None,
                end_style: LineEnd::ClosedArrow,
            },
            AnnotKind::Line,
        ),
        (
            Geometry::Square {
                rect: [300.0, 100.0, 400.0, 180.0],
            },
            AnnotKind::Square,
        ),
        (
            Geometry::Circle {
                rect: [300.0, 200.0, 400.0, 280.0],
            },
            AnnotKind::Circle,
        ),
        (
            Geometry::Polygon {
                points: vec![[300.0, 500.0], [400.0, 500.0], [350.0, 580.0]],
            },
            AnnotKind::Polygon,
        ),
        (
            Geometry::PolyLine {
                points: vec![[300.0, 600.0], [350.0, 640.0], [400.0, 600.0]],
            },
            AnnotKind::PolyLine,
        ),
        (
            Geometry::Ink {
                strokes: vec![
                    vec![[50.0, 600.0], [80.0, 640.0], [110.0, 600.0]],
                    vec![[60.0, 700.0], [90.0, 700.0]],
                ],
            },
            AnnotKind::Ink,
        ),
    ]
}

#[test]
fn every_kind_roundtrips_with_properties() {
    let mut doc = blank();
    let mut ids = Vec::new();
    for (g, k) in geometries() {
        let id = add(&mut doc, &styled(AnnotSpec::new(0, g.clone()))).unwrap();
        ids.push((id, g, k));
    }
    let mut doc = reopen(&doc);
    let all = list(&mut doc, None).unwrap();
    assert_eq!(all.len(), ids.len());
    for (info, (_, g, k)) in all.iter().zip(&ids) {
        assert_eq!(info.kind, *k, "{g:?}");
        assert_eq!(info.author.as_deref(), Some("Ana"));
        assert_eq!(info.subject.as_deref(), Some("Revisión"));
        assert_eq!(info.contents.as_deref(), Some("Hola ñandú"), "{k:?}");
        assert!(info.created.as_deref().unwrap().starts_with("D:20"));
        assert!(near(info.style.opacity.unwrap(), 0.5), "{k:?}");
        let c = info.style.color.unwrap_or_default();
        if !matches!(k, AnnotKind::Note | AnnotKind::FreeText) {
            assert!(near(c[0], 1.0) && near(c[1], 0.0), "{k:?} color {c:?}");
        }
        match g {
            Geometry::Highlight { quads }
            | Geometry::Underline { quads }
            | Geometry::StrikeOut { quads }
            | Geometry::Squiggly { quads } => {
                assert_eq!(info.quads.len(), quads.len());
                for (a, b) in info.quads[0].iter().zip(&quads[0]) {
                    assert!(near(*a, *b), "{:?} vs {:?}", info.quads, quads);
                }
            }
            Geometry::Polygon { points } | Geometry::PolyLine { points } => {
                assert_eq!(info.points.len(), points.len());
                for (a, b) in info.points.iter().zip(points) {
                    assert!(near(a[0], b[0]) && near(a[1], b[1]));
                }
            }
            Geometry::Ink { strokes } => {
                assert_eq!(info.strokes.len(), strokes.len());
                assert_eq!(info.strokes[0].len(), strokes[0].len());
            }
            Geometry::Line { start, end, .. } => {
                let (s, e) = info.line.unwrap();
                assert!(near(s[0], start[0]) && near(s[1], start[1]) && near(e[0], end[0]) && near(e[1], end[1]));
            }
            _ => {}
        }
    }
}

#[test]
fn update_changes_properties_and_moves() {
    let mut doc = blank();
    let id = add(
        &mut doc,
        &AnnotSpec::new(
            1,
            Geometry::Square {
                rect: [100.0, 100.0, 200.0, 200.0],
            },
        ),
    )
    .unwrap();
    update(
        &mut doc,
        id,
        &AnnotPatch {
            style: Style {
                color: Some([0.0, 0.0, 1.0]),
                opacity: Some(0.25),
                border_width: Some(3.0),
                ..Default::default()
            },
            author: Some("Luis".into()),
            contents: Some("movido".into()),
            move_by: Some([10.0, 20.0]),
            ..Default::default()
        },
    )
    .unwrap();
    let mut doc = reopen(&doc);
    let info = &list(&mut doc, Some(1)).unwrap()[0];
    assert_eq!(info.author.as_deref(), Some("Luis"));
    assert_eq!(info.contents.as_deref(), Some("movido"));
    assert!(near(info.style.opacity.unwrap(), 0.25));
    assert!(near(info.style.border_width.unwrap(), 3.0));
    // El recuadro se desplaza (10, 20); el motor lo ensancha lo que mide el borde, para que lo cubra.
    assert!(
        (info.rect[0] - 110.0).abs() <= 3.0
            && (info.rect[1] - 120.0).abs() <= 3.0
            && info.rect[0] <= 110.0
            && info.rect[1] <= 120.0,
        "{:?}",
        info.rect
    );
    assert!(info.modified.is_some());
}

#[test]
fn replies_state_and_delete_cascade() {
    let mut doc = blank();
    let parent = add(
        &mut doc,
        &styled(AnnotSpec::new(0, Geometry::Note { at: [40.0, 40.0] })),
    )
    .unwrap();
    let mut reply = AnnotSpec::new(0, Geometry::Note { at: [40.0, 40.0] });
    reply.reply_to = Some(parent);
    reply.contents = Some("De acuerdo".into());
    reply.state = Some(ReviewState::Accepted);
    let r = add(&mut doc, &reply).unwrap();

    let mut doc = reopen(&doc);
    let all = list(&mut doc, Some(0)).unwrap();
    let ri = all.iter().find(|i| i.id.xref == r.xref).unwrap();
    assert_eq!(ri.reply_to, Some(parent.xref));
    assert_eq!(ri.state, Some(ReviewState::Accepted));

    delete(&mut doc, parent).unwrap();
    assert!(list(&mut doc, None).unwrap().is_empty());
}

#[test]
fn bad_page_and_missing_annotation_are_errors() {
    let mut doc = blank();
    assert!(add(&mut doc, &AnnotSpec::new(9, Geometry::Note { at: [0.0, 0.0] })).is_err());
    assert!(delete(&mut doc, AnnotId { page: 0, xref: 9999 }).is_err());
}

#[test]
fn spec_is_serializable() {
    let (g, _) = geometries().remove(6);
    let spec = styled(AnnotSpec::new(0, g));
    let json = serde_json::to_string(&spec).unwrap();
    assert_eq!(serde_json::from_str::<AnnotSpec>(&json).unwrap(), spec);
}

#[test]
fn appearances_are_generated() {
    let mut doc = blank();
    for (g, _) in geometries() {
        add(&mut doc, &styled(AnnotSpec::new(0, g))).unwrap();
    }
    let doc = reopen(&doc);
    let page = doc.load_pdf_page(0).unwrap();
    let mut n = 0;
    for a in page.annotations() {
        assert!(
            a.object().get_dict("AP").unwrap().is_some(),
            "{:?} sin apariencia",
            a.r#type()
        );
        n += 1;
    }
    assert_eq!(n, 12);
}

#[test]
fn nm_is_unique_and_survives_save() {
    let mut doc = blank();
    for _ in 0..3 {
        add(&mut doc, &AnnotSpec::new(0, Geometry::Note { at: [10.0, 10.0] })).unwrap();
    }
    let mut doc = reopen(&doc);
    let nms: std::collections::HashSet<_> = list(&mut doc, None)
        .unwrap()
        .into_iter()
        .map(|i| i.nm.unwrap())
        .collect();
    assert_eq!(nms.len(), 3);
    assert!(nms.iter().all(|n| n.len() == 36 && n.as_bytes()[14] == b'4'));
}

#[test]
fn flatten_removes_annotations_but_keeps_ink_on_page() {
    let mut doc = blank();
    for (g, _) in geometries() {
        add(&mut doc, &styled(AnnotSpec::new(0, g))).unwrap();
    }
    assert_eq!(flatten(&mut doc).unwrap(), 12);
    let mut doc = reopen(&doc);
    assert!(list(&mut doc, None).unwrap().is_empty());
    assert_eq!(flatten(&mut doc).unwrap(), 0);
    // La página ahora tiene contenido (antes estaba en blanco).
    let page = doc.load_pdf_page(0).unwrap();
    assert!(page.contents().unwrap().is_some());
}

#[test]
fn summary_groups_threads() {
    let mut doc = blank();
    let p = add(
        &mut doc,
        &styled(AnnotSpec::new(0, Geometry::Note { at: [40.0, 40.0] })),
    )
    .unwrap();
    let mut reply = AnnotSpec::new(0, Geometry::Note { at: [40.0, 40.0] });
    reply.reply_to = Some(p);
    reply.author = Some("Luis".into());
    reply.contents = Some("De acuerdo".into());
    reply.state = Some(ReviewState::Accepted);
    add(&mut doc, &reply).unwrap();
    add(
        &mut doc,
        &AnnotSpec::new(
            1,
            Geometry::Square {
                rect: [10.0, 10.0, 50.0, 50.0],
            },
        ),
    )
    .unwrap();
    let s = summary(&mut doc).unwrap();
    assert_eq!(s.total, 3);
    assert_eq!(s.threads.len(), 2);
    assert_eq!(s.threads[0].replies.len(), 1);
    assert_eq!(s.by_author["Ana"], 1);
    let t = s.to_text();
    assert!(t.contains("3 comentario(s)") && t.contains("[aceptado]"), "{t}");
}

#[test]
fn xfdf_roundtrip_between_documents() {
    let mut a = blank();
    let mut ids = Vec::new();
    for (g, _) in geometries() {
        ids.push(add(&mut a, &styled(AnnotSpec::new(0, g))).unwrap());
    }
    let mut reply = AnnotSpec::new(0, Geometry::Note { at: [40.0, 40.0] });
    reply.reply_to = Some(ids[0]);
    reply.contents = Some("Acuerdo <&> \"ok\"".into());
    reply.state = Some(ReviewState::Rejected);
    add(&mut a, &reply).unwrap();
    let xml = export_xfdf(&mut a).unwrap();
    assert!(xml.contains("<highlight") && xml.contains("inreplyto="), "{xml}");

    let mut b = blank();
    assert_eq!(import_xfdf(&mut b, &xml).unwrap(), 13);
    // Reimportar no duplica.
    assert_eq!(import_xfdf(&mut b, &xml).unwrap(), 0);

    let la = list(&mut a, None).unwrap();
    let lb = list(&mut b, None).unwrap();
    assert_eq!(la.len(), lb.len());
    for x in &la {
        let y = lb
            .iter()
            .find(|y| y.nm == x.nm)
            .unwrap_or_else(|| panic!("falta {:?}", x.nm));
        assert_eq!(x.kind, y.kind);
        assert_eq!(x.contents, y.contents);
        assert_eq!(x.author, y.author);
        assert_eq!(x.state, y.state);
        assert_eq!(x.created, y.created);
        // MuPDF ensancha el rect del texto libre según el borde al regenerar la apariencia: tolerancia de 3 pt.
        assert!(
            (x.rect[0] - y.rect[0]).abs() < 3.0 && (x.rect[1] - y.rect[1]).abs() < 3.0,
            "{:?}: {:?} vs {:?}",
            x.kind,
            x.rect,
            y.rect
        );
        assert_eq!(x.reply_to.is_some(), y.reply_to.is_some());
    }
}

#[test]
fn xfdf_rejects_garbage() {
    let mut d = blank();
    assert!(import_xfdf(&mut d, "<html></html>").is_err());
    assert!(import_xfdf(&mut d, "<xfdf><annots><square page=\"0\"></xfdf>").is_err());
}
