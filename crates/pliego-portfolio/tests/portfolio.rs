use std::path::PathBuf;

use pliego_core::JobContext;

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

#[test]
fn create_list_and_extract_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let notes = dir.path().join("notas.txt");
    std::fs::write(&notes, "hola cartera").unwrap();
    let out = dir.path().join("cartera.pdf");
    let files = vec![
        corpus("organize/a.pdf"),
        notes.clone(),
        corpus("compare/base.pdf"),
        corpus("organize/a.pdf"),
    ];
    let ctx = JobContext::silent();

    pliego_portfolio::create(&files, &out, &ctx).unwrap();
    let l = pliego_portfolio::list(&out, None).unwrap();
    assert!(l.is_portfolio);
    let names: Vec<_> = l.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names.len(), 4);
    assert!(
        names.contains(&"a.pdf") && names.contains(&"a (2).pdf") && names.contains(&"notas.txt"),
        "{names:?}"
    );
    let txt = l.entries.iter().find(|e| e.name == "notas.txt").unwrap();
    assert_eq!((txt.size, txt.mime_type.as_deref()), (12, Some("text/plain")));

    // Extraer uno y luego todo, sin pisar lo existente.
    let dest = dir.path().join("fuera");
    let r = pliego_portfolio::extract(&out, &dest, &["notas.txt".into()], None, &ctx).unwrap();
    assert_eq!(std::fs::read_to_string(&r.outputs[0]).unwrap(), "hola cartera");
    let r = pliego_portfolio::extract(&out, &dest, &[], None, &ctx).unwrap();
    assert_eq!(r.outputs.len(), 4);
    assert!(dest.join("notas (2).txt").exists());
    assert_eq!(
        std::fs::read(dest.join("base.pdf")).unwrap(),
        std::fs::read(corpus("compare/base.pdf")).unwrap()
    );
}

#[test]
fn errors_are_friendly() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = JobContext::silent();
    assert!(pliego_portfolio::create(&[], &dir.path().join("x.pdf"), &ctx).is_err());
    let out = dir.path().join("c.pdf");
    pliego_portfolio::create(&[corpus("organize/a.pdf")], &out, &ctx).unwrap();
    let e = pliego_portfolio::extract(&out, dir.path(), &["no-existe.txt".into()], None, &ctx).unwrap_err();
    assert!(e.to_string().contains("no contiene"));
    // Un PDF normal no es cartera.
    assert!(
        !pliego_portfolio::list(&corpus("organize/a.pdf"), None)
            .unwrap()
            .is_portfolio
    );
}
