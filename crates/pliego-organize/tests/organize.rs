mod common;

use common::*;
use pliego_core::JobContext;
use pliego_organize::*;

#[test]
fn merge_keeps_order_and_bookmarks() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_pdf(dir.path(), "a.pdf", "A", 2);
    let b = write_pdf(dir.path(), "b.pdf", "B", 3);
    let out = dir.path().join("m.pdf");
    let rep = merge(&[a, b], &out, &MergeOptions::default(), &JobContext::silent()).unwrap();
    assert_eq!(rep.outputs, vec![out.clone()]);
    let doc = open_pdf(&out);
    assert_eq!(texts(&doc), ["A p1", "A p2", "B p1", "B p2", "B p3"]);
    let marks = bookmark_targets(&doc);
    assert_eq!(marks.len(), 5);
    assert_eq!(marks[0], ("A m1".into(), "A p1".into()));
    assert_eq!(marks[3], ("B m2".into(), "B p2".into()));
}

#[test]
fn merge_bookmark_per_file() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_pdf(dir.path(), "a.pdf", "A", 1);
    let b = write_pdf(dir.path(), "b.pdf", "B", 2);
    let out = dir.path().join("m.pdf");
    let opts = MergeOptions {
        bookmark_per_file: true,
        ..Default::default()
    };
    merge(&[a, b], &out, &opts, &JobContext::silent()).unwrap();
    let outline = read_outline(&open_pdf(&out)).unwrap();
    assert_eq!(outline.len(), 2);
    assert_eq!(outline[1].title, "b");
    assert_eq!(outline[1].page, Some(1));
    assert_eq!(outline[1].children.len(), 2);
    assert_eq!(outline[1].children[1].page, Some(2));
}

#[test]
fn merge_missing_input_fails() {
    let dir = tempfile::tempdir().unwrap();
    let r = merge(
        &[dir.path().join("no.pdf")],
        &dir.path().join("o.pdf"),
        &MergeOptions::default(),
        &JobContext::silent(),
    );
    assert!(r.is_err());
    assert!(!dir.path().join("o.pdf").exists());
}

fn sample(n: usize) -> (tempfile::TempDir, pliego_organize::PdfDocument) {
    let dir = tempfile::tempdir().unwrap();
    let p = write_pdf(dir.path(), "s.pdf", "S", n);
    let doc = open_pdf(&p);
    (dir, doc)
}

#[test]
fn extract_in_given_order_with_bookmarks() {
    let (_d, doc) = sample(5);
    let out = extract(&doc, &[3, 0, 4]).unwrap();
    assert_eq!(texts(&out), ["S p4", "S p1", "S p5"]);
    let marks = bookmark_targets(&out);
    assert_eq!(marks.len(), 3);
    for (title, text) in marks {
        assert_eq!(title.replace('m', "p"), text);
    }
    assert!(extract(&doc, &[9]).is_err());
}

#[test]
fn delete_and_protect_last() {
    let (_d, mut doc) = sample(4);
    delete(&mut doc, &[1, 2]).unwrap();
    assert_eq!(texts(&doc), ["S p1", "S p4"]);
    assert!(delete(&mut doc, &[0, 1]).is_err());
}

#[test]
fn rotate_adds_to_current() {
    let (_d, mut doc) = sample(2);
    rotate(&mut doc, &[1], 90).unwrap();
    rotate(&mut doc, &[1], 270 + 90).unwrap();
    let doc = reopen(&doc);
    assert_eq!(doc.load_pdf_page(0).unwrap().rotation().unwrap(), 0);
    assert_eq!(doc.load_pdf_page(1).unwrap().rotation().unwrap(), 90);
    let mut doc = doc;
    assert!(rotate(&mut doc, &[0], 45).is_err());
}

#[test]
fn reorder_keeps_bookmarks_on_their_pages() {
    let (_d, mut doc) = sample(4);
    reorder(&mut doc, &[3, 1, 0, 2]).unwrap();
    assert_eq!(texts(&doc), ["S p4", "S p2", "S p1", "S p3"]);
    let marks = bookmark_targets(&reopen(&doc));
    assert_eq!(marks.len(), 4);
    for (title, text) in marks {
        assert_eq!(title.replace('m', "p"), text);
    }
    assert!(reorder(&mut doc, &[0, 0, 1, 2]).is_err());
    assert!(reorder(&mut doc, &[0, 1]).is_err());
}

#[test]
fn insert_blank_and_from_other() {
    let (d, mut doc) = sample(2);
    insert_blank(&mut doc, 1, 2, None).unwrap();
    assert_eq!(doc.page_count().unwrap(), 4);
    assert_eq!(page_text(&doc, 1), "");
    let other = open_pdf(&write_pdf(d.path(), "o.pdf", "O", 3));
    insert_from(&mut doc, &other, &[2, 0], 4).unwrap();
    assert_eq!(texts(&doc), ["S p1", "", "", "S p2", "O p3", "O p1"]);
    let b = doc.load_page(1).unwrap().bounds().unwrap();
    assert_eq!((b.width(), b.height()), (595.0, 842.0));
}

#[test]
fn duplicate_and_replace_and_move() {
    let (d, mut doc) = sample(3);
    duplicate(&mut doc, &[0, 2]).unwrap();
    assert_eq!(texts(&doc), ["S p1", "S p1", "S p2", "S p3", "S p3"]);
    let other = open_pdf(&write_pdf(d.path(), "o.pdf", "O", 1));
    replace(&mut doc, 1, 2, &other, &[0]).unwrap();
    assert_eq!(texts(&doc), ["S p1", "O p1", "S p3", "S p3"]);
    move_page(&mut doc, 0, 3).unwrap();
    assert_eq!(texts(&doc), ["O p1", "S p3", "S p3", "S p1"]);
}

#[test]
fn crop_rect_and_margins() {
    let (_d, mut doc) = sample(1);
    crop(
        &mut doc,
        &[0],
        CropSpec::Rect {
            x0: 10.0,
            y0: 20.0,
            x1: 310.0,
            y1: 420.0,
        },
    )
    .unwrap();
    let b = doc.load_page(0).unwrap().bounds().unwrap();
    assert_eq!((b.width(), b.height()), (300.0, 400.0));
    crop(
        &mut doc,
        &[0],
        CropSpec::Margins {
            left: 50.0,
            top: 0.0,
            right: 0.0,
            bottom: 100.0,
        },
    )
    .unwrap();
    let b = doc.load_page(0).unwrap().bounds().unwrap();
    assert_eq!((b.width(), b.height()), (250.0, 300.0));
    assert!(
        crop(
            &mut doc,
            &[0],
            CropSpec::Rect {
                x0: 5.0,
                y0: 5.0,
                x1: 5.0,
                y1: 9.0
            }
        )
        .is_err()
    );
}

#[test]
fn page_labels() {
    let (_d, mut doc) = sample(5);
    set_page_labels(
        &mut doc,
        &[
            PageLabelSpec {
                start_page: 0,
                style: LabelStyle::LowerRoman,
                prefix: String::new(),
                first_number: 1,
            },
            PageLabelSpec {
                start_page: 2,
                style: LabelStyle::Decimal,
                prefix: "A-".into(),
                first_number: 1,
            },
        ],
    )
    .unwrap();
    let doc = reopen(&doc);
    assert_eq!(doc.page_label(1).unwrap(), "ii");
    assert_eq!(doc.page_label(2).unwrap(), "A-1");
    assert_eq!(doc.page_label(4).unwrap(), "A-3");
}

#[test]
fn split_every_n_ranges_and_bookmarks() {
    let (d, doc) = sample(5);
    let out = d.path().join("out");
    let ctx = JobContext::silent();
    let rep = split(&doc, "s", &SplitRule::EveryN { n: 2 }, &out, &ctx).unwrap();
    assert_eq!(rep.outputs.len(), 3);
    assert_eq!(texts(&open_pdf(&rep.outputs[2])), ["S p5"]);
    assert_eq!(texts(&open_pdf(&rep.outputs[0])), ["S p1", "S p2"]);
    // Marcadores del tramo apuntan a sus páginas dentro de cada parte.
    assert_eq!(
        bookmark_targets(&open_pdf(&rep.outputs[1])),
        vec![("S m3".into(), "S p3".into()), ("S m4".into(), "S p4".into())]
    );

    let rep = split(
        &doc,
        "r",
        &SplitRule::Ranges {
            ranges: vec![(0, 0), (2, 4)],
        },
        &out,
        &ctx,
    )
    .unwrap();
    assert_eq!(texts(&open_pdf(&rep.outputs[1])), ["S p3", "S p4", "S p5"]);
    assert!(split(&doc, "r", &SplitRule::Ranges { ranges: vec![(3, 9)] }, &out, &ctx).is_err());

    let rep = split(&doc, "b", &SplitRule::Bookmarks, &out, &ctx).unwrap();
    assert_eq!(rep.outputs.len(), 5);
    assert!(
        rep.outputs[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("b-001-S_m1")
    );
}

#[test]
fn split_by_max_size() {
    let (d, doc) = sample(20);
    let mut one = Vec::new();
    extract(&doc, &[0]).unwrap().write_to(&mut one).unwrap();
    let limit = (one.len() as u64) * 5;
    let rep = split(
        &doc,
        "z",
        &SplitRule::MaxBytes { bytes: limit },
        &d.path().join("o"),
        &JobContext::silent(),
    )
    .unwrap();
    assert!(
        rep.outputs.len() > 1 && rep.outputs.len() < 20,
        "{} partes",
        rep.outputs.len()
    );
    let mut all = Vec::new();
    for p in &rep.outputs {
        assert!(std::fs::metadata(p).unwrap().len() <= limit, "{p:?} supera el límite");
        all.extend(texts(&open_pdf(p)));
    }
    assert_eq!(all.len(), 20);
    assert_eq!(all[19], "S p20");
}

#[test]
fn save_in_place_is_atomic() {
    let (d, doc) = sample(2);
    let path = d.path().join("s.pdf");
    save(&doc, &path).unwrap();
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
}

#[test]
fn crop_margins_on_rotated_page_use_user_space() {
    let (_d, mut doc) = sample(1);
    rotate(&mut doc, &[0], 90).unwrap();
    // Márgenes en el espacio de usuario sin girar: 595 x 842 -> 495 x 742.
    crop(
        &mut doc,
        &[0],
        CropSpec::Margins {
            left: 50.0,
            top: 50.0,
            right: 50.0,
            bottom: 50.0,
        },
    )
    .unwrap();
    let doc = reopen(&doc);
    let b = doc.load_page(0).unwrap().bounds().unwrap();
    // Visualmente girada: ancho y alto intercambiados.
    assert_eq!((b.width(), b.height()), (742.0, 495.0));
}

/// Cifra un PDF de prueba con AES-256 y `user` como contraseña de apertura.
fn write_encrypted(dir: &std::path::Path, name: &str, label: &str, pages: usize, user: &str) -> std::path::PathBuf {
    use pliego_pdf::{Encryption, PdfDocument, WriteOptions};
    let doc = PdfDocument::from_bytes(&build_pdf(label, pages)).unwrap();
    let mut o = WriteOptions::new();
    o.set_encryption(Encryption::Aes256)
        .set_user_password(user)
        .set_owner_password("dueño");
    let path = dir.join(name);
    std::fs::write(&path, doc.to_bytes(&o).unwrap()).unwrap();
    path
}

#[test]
fn merge_uses_the_password_of_each_protected_file() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_encrypted(dir.path(), "a.pdf", "A", 2, "clave-a");
    let b = write_encrypted(dir.path(), "b.pdf", "B", 1, "clave-b");
    let c = write_pdf(dir.path(), "c.pdf", "C", 1);
    let out = dir.path().join("m.pdf");
    let ctx = JobContext::silent();
    let inputs = [a.clone(), b.clone(), c];

    // Sin contraseñas, o con una común que sirve solo a uno de los dos, no se puede.
    assert!(matches!(
        merge(&inputs, &out, &MergeOptions::default(), &ctx),
        Err(pliego_core::Error::Password)
    ));
    let common = MergeOptions {
        password: Some("clave-a".into()),
        ..Default::default()
    };
    assert!(matches!(
        merge(&inputs, &out, &common, &ctx),
        Err(pliego_core::Error::Password)
    ));
    assert!(!out.exists());

    // Una por archivo; la común sigue valiendo de reserva para los que no tienen la suya.
    let each = MergeOptions {
        password: Some("clave-b".into()),
        file_passwords: [(a, "clave-a".to_string())].into(),
        ..Default::default()
    };
    merge(&inputs, &out, &each, &ctx).unwrap();
    assert_eq!(texts(&open_pdf(&out)), ["A p1", "A p2", "B p1", "C p1"]);
}

#[test]
fn merge_options_read_the_per_file_passwords_from_json() {
    let o: MergeOptions = serde_json::from_str(
        r#"{"keep_bookmarks":true,"file_passwords":{"/tmp/a b.pdf":"uno","C:\\Docs\\b.pdf":"dos"}}"#,
    )
    .unwrap();
    assert_eq!(o.file_passwords.len(), 2);
    assert_eq!(o.file_passwords[std::path::Path::new("/tmp/a b.pdf")], "uno");
    assert!(MergeOptions::default().file_passwords.is_empty());
}
