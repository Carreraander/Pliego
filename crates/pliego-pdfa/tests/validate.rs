mod common;

use common::{Opts, build, write, xmp};
use pliego_pdfa::{Level, Rule, validate};

fn rules(o: &Opts, part: u8, conf: &str, level: Level) -> Vec<Rule> {
    let dir = tempfile::tempdir().unwrap();
    let p = write(dir.path(), "t.pdf", &build(o, part, conf));
    validate(&p, level).unwrap().issues.iter().map(|i| i.rule).collect()
}

#[test]
fn conforming_documents_pass() {
    for (part, conf, level) in [
        (1, "B", Level::A1b),
        (2, "B", Level::A2b),
        (3, "B", Level::A3b),
        (2, "U", Level::A2b),
    ] {
        let r = rules(&Opts::default(), part, conf, level);
        assert!(r.is_empty(), "{level:?}: {r:?}");
    }
}

#[test]
fn level_mismatch_is_reported() {
    assert_eq!(
        rules(&Opts::default(), 1, "B", Level::A2b),
        vec![Rule::XmpLevelMismatch]
    );
    // un 2b declarado no es 2u
    assert!(rules(&Opts::default(), 2, "B", Level::A2u).contains(&Rule::XmpLevelMismatch));
}

#[test]
fn missing_metadata_and_intent() {
    let r = rules(
        &Opts {
            no_xmp: true,
            no_intent: true,
            ..Default::default()
        },
        2,
        "B",
        Level::A2b,
    );
    assert!(r.contains(&Rule::NoXmp) && r.contains(&Rule::NoOutputIntent), "{r:?}");
    let r = rules(
        &Opts {
            xmp: Some("<x:xmpmeta/>".into()),
            ..Default::default()
        },
        2,
        "B",
        Level::A2b,
    );
    assert_eq!(r, vec![Rule::XmpWithoutPdfaId]);
}

#[test]
fn fonts_javascript_and_filters() {
    let r = rules(
        &Opts {
            helvetica_not_embedded: true,
            ..Default::default()
        },
        2,
        "B",
        Level::A2b,
    );
    assert_eq!(r, vec![Rule::FontNotEmbedded]);
    let r = rules(
        &Opts {
            javascript: true,
            ..Default::default()
        },
        2,
        "B",
        Level::A2b,
    );
    assert_eq!(r, vec![Rule::ForbiddenAction]);
    let r = rules(
        &Opts {
            lzw: true,
            ..Default::default()
        },
        2,
        "B",
        Level::A2b,
    );
    assert_eq!(r, vec![Rule::LzwFilter]);
}

#[test]
fn transparency_only_matters_in_part_1() {
    let o = Opts {
        transparency: true,
        ..Default::default()
    };
    assert_eq!(rules(&o, 1, "B", Level::A1b), vec![Rule::Transparency]);
    assert!(rules(&o, 2, "B", Level::A2b).is_empty());
    let o = Opts {
        ocg: true,
        ..Default::default()
    };
    assert_eq!(rules(&o, 1, "B", Level::A1b), vec![Rule::OptionalContent]);
    assert!(rules(&o, 3, "B", Level::A3b).is_empty());
}

#[test]
fn attachments_only_in_part_3() {
    let o = Opts {
        embedded_file: true,
        af: true,
        ..Default::default()
    };
    assert_eq!(rules(&o, 2, "B", Level::A2b), vec![Rule::EmbeddedFile]);
    assert!(rules(&o, 3, "B", Level::A3b).is_empty());
    let o = Opts {
        embedded_file: true,
        af: false,
        ..Default::default()
    };
    assert_eq!(rules(&o, 3, "B", Level::A3b), vec![Rule::AttachmentWithoutRelationship]);
}

#[test]
fn annotations_must_be_printable_with_appearance() {
    let o = Opts {
        hidden_annot: true,
        ..Default::default()
    };
    let r = rules(&o, 2, "B", Level::A2b);
    assert!(
        r.contains(&Rule::AnnotationNotPrintable) && r.contains(&Rule::AnnotationWithoutAppearance),
        "{r:?}"
    );
}

#[test]
fn unicode_level_needs_tounicode() {
    let o = Opts {
        type0_without_tounicode: true,
        ..Default::default()
    };
    let r = rules(&o, 2, "U", Level::A2u);
    assert!(r.contains(&Rule::FontWithoutUnicode), "{r:?}");
    assert!(!rules(&o, 2, "B", Level::A2b).contains(&Rule::FontWithoutUnicode));
}

#[test]
fn xmp_helper_builds_valid_packet() {
    assert!(xmp(1, "B").contains("<pdfaid:part>1</pdfaid:part>"));
}

#[test]
fn issues_serialize_with_stable_codes() {
    let dir = tempfile::tempdir().unwrap();
    let p = write(
        dir.path(),
        "t.pdf",
        &build(
            &Opts {
                no_xmp: true,
                ..Default::default()
            },
            2,
            "B",
        ),
    );
    let v = validate(&p, Level::A2b).unwrap();
    let j = serde_json::to_value(&v).unwrap();
    assert_eq!(j["level"], "2b");
    assert_eq!(j["issues"][0]["rule"], "no_xmp");
    assert!(!v.conforms());
}
