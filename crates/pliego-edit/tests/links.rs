use pliego_document::Document;
use pliego_edit::*;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/edit/parrafos.pdf");

fn uri(u: &str) -> LinkTarget {
    LinkTarget::Uri { uri: u.into() }
}

#[test]
fn list_update_and_delete_links() {
    let mut doc = Document::open(CORPUS, None).unwrap();
    assert!(links(&doc, 0).unwrap().is_empty());
    add_link(&mut doc, 0, [50.0, 100.0, 200.0, 120.0], &uri("https://pdfpliego.com")).unwrap();
    add_link(&mut doc, 0, [50.0, 300.0, 200.0, 320.0], &LinkTarget::Page { page: 0 }).unwrap();
    let l = links(&doc, 0).unwrap();
    assert_eq!(l.len(), 2);
    assert_eq!(l[0].target, Some(uri("https://pdfpliego.com")));
    assert_eq!(l[1].target, Some(LinkTarget::Page { page: 0 }));
    for (got, want) in l[0].rect.iter().zip([50.0, 100.0, 200.0, 120.0]) {
        assert!((got - want).abs() < 0.01, "{:?}", l[0].rect);
    }

    // Cambiar el destino conserva el recuadro.
    update_link(&mut doc, 0, l[0].id, &uri("mailto:hola@pdfpliego.com")).unwrap();
    let l = links(&doc, 0).unwrap();
    assert_eq!(l.len(), 2);
    assert!(
        l.iter()
            .any(|x| x.target == Some(uri("mailto:hola@pdfpliego.com")) && (x.rect[1] - 100.0).abs() < 0.01)
    );

    // Borrar uno deja el otro.
    let first = l.iter().find(|x| x.rect[1] < 200.0).unwrap().id;
    delete_link(&mut doc, 0, first).unwrap();
    let l = links(&doc, 0).unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].target, Some(LinkTarget::Page { page: 0 }));
    // Lo que no es un enlace da error.
    assert!(delete_link(&mut doc, 0, 99).is_err());
}

#[test]
fn invalid_update_keeps_the_old_link() {
    let mut doc = Document::open(CORPUS, None).unwrap();
    add_link(&mut doc, 0, [50.0, 100.0, 200.0, 120.0], &uri("https://a.example")).unwrap();
    let id = links(&doc, 0).unwrap()[0].id;
    assert!(update_link(&mut doc, 0, id, &LinkTarget::Page { page: 9 }).is_err());
    let l = links(&doc, 0).unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].target, Some(uri("https://a.example")));
}
