use std::path::PathBuf;

use super::*;

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/basic")
        .join(name)
}

fn open() -> DefaultDocument {
    DefaultEngine.open(&corpus("hola.pdf"), None).expect("abre hola.pdf")
}

#[test]
fn info_and_pages() {
    let d = open();
    let info = d.info().unwrap();
    assert_eq!(info.page_count, 2);
    assert_eq!(info.title.as_deref(), Some("Documento de prueba"));
    assert!(info.is_pdf);
    let s = d.page_size(0).unwrap();
    assert_eq!((s.width.round(), s.height.round()), (595.0, 842.0));
    assert!(d.page_size(2).is_err());
}

#[test]
fn text_search_outline() {
    let d = open();
    assert!(d.page_text(0).unwrap().contains("Hola Pliego"));
    assert_eq!(d.search_page(1, "Pliego", 10).unwrap().quads.len(), 1);
    let o = d.outline().unwrap();
    assert_eq!(o.len(), 2);
    assert_eq!(o[1].title, "Segunda");
    assert_eq!(o[1].page, Some(1));
}

#[test]
fn render_and_tiles_match() {
    let d = open();
    let full = d.render_page(0, 0.5).unwrap();
    assert_eq!((full.width, full.height), (298, 421));
    assert_eq!(full.data.len(), (298 * 421 * 4) as usize);
    // La tesela (1,0) de 128 px debe coincidir con la región equivalente de la página completa.
    let t = d
        .render_tile(TileRequest {
            page: 0,
            zoom: 0.5,
            col: 1,
            row: 0,
            size: 128,
        })
        .unwrap();
    assert_eq!((t.width, t.height), (128, 128));
    let mut diff = 0u64;
    for y in 0..128usize {
        for x in 0..128usize {
            let a = &t.data[(y * 128 + x) * 4..][..3];
            let b = &full.data[(y * 298 + 128 + x) * 4..][..3];
            diff += a.iter().zip(b).map(|(a, b)| a.abs_diff(*b) as u64).sum::<u64>();
        }
    }
    assert!(diff < 128 * 128 * 3, "diferencia media > 1 nivel: {diff}");
    // Tesela del borde: recortada.
    let edge = d
        .render_tile(TileRequest {
            page: 0,
            zoom: 0.5,
            col: 2,
            row: 3,
            size: 128,
        })
        .unwrap();
    assert_eq!((edge.width, edge.height), (298 - 256, 421 - 384));
    assert!(
        d.render_tile(TileRequest {
            page: 0,
            zoom: 0.5,
            col: 9,
            row: 0,
            size: 128
        })
        .is_err()
    );
}

#[test]
fn png_roundtrip() {
    let d = open();
    let img = d.render_page(0, 0.25).unwrap();
    let png = encode_png(&img).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn missing_file_is_io_error() {
    assert!(matches!(
        DefaultEngine.open(&corpus("no-existe.pdf"), None),
        Err(Error::Io { .. })
    ));
}

#[test]
fn hostile_outline_does_not_crash() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/outline/hostil.pdf");
    let d = DefaultEngine.open(&path, None).expect("abre hostil.pdf");
    let o = d.outline().unwrap();
    // Se corta en cuanto un nodo ya visitado reaparece (ciclos por /Next y por /First).
    assert_eq!(o.len(), 3, "{o:#?}");
    assert_eq!((o[0].title.as_str(), o[0].page), ("Inicio", Some(0)));
    assert_eq!(
        (o[1].title.as_str(), o[1].page),
        ("", Some(1)),
        "sin /Title y con destino con nombre"
    );
    assert_eq!(o[2].title, "Ciclo");
    assert_eq!(o[2].children.len(), 1);
    assert_eq!(o[2].children[0].title, "Hijo");
    assert_eq!(o[2].children[0].page, Some(1));
    assert!(o[2].children[0].children.is_empty());
}

#[test]
fn open_does_not_keep_the_file_open() {
    // Abierto desde memoria: el archivo puede sustituirse (en Windows, renombrar encima fallaba con «Access is
    // denied» mientras el worker lo tenía abierto) y el documento ya abierto sigue entero.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.pdf");
    std::fs::copy(corpus("hola.pdf"), &path).unwrap();
    let d = DefaultEngine.open(&path, None).unwrap();
    std::fs::write(&path, b"basura").unwrap();
    assert!(d.page_text(0).unwrap().contains("Hola Pliego"));
    assert_eq!(d.info().unwrap().page_count, 2);
}

#[test]
fn open_missing_file_reports_the_path() {
    let e = DefaultEngine.open(Path::new("/no/existe.pdf"), None).err().unwrap();
    assert!(matches!(e, Error::Io { .. }), "{e}");
    assert!(e.to_string().contains("existe.pdf"), "{e}");
}

#[test]
fn view_only_reads_like_a_normal_open() {
    let (v, n) = (
        DefaultEngine.open_for_viewing(&corpus("hola.pdf"), None).unwrap(),
        open(),
    );
    assert_eq!(v.info().unwrap(), n.info().unwrap());
    assert_eq!(v.page_size(1).unwrap(), n.page_size(1).unwrap());
    assert_eq!(v.page_text(0).unwrap(), n.page_text(0).unwrap());
    assert_eq!(v.outline().unwrap(), n.outline().unwrap());
    assert_eq!(v.render_page(0, 0.5).unwrap(), n.render_page(0, 0.5).unwrap());
    assert_eq!(
        v.search_page(1, "Pliego", 10).unwrap(),
        n.search_page(1, "Pliego", 10).unwrap()
    );
}

#[test]
fn open_auto_uses_view_only_from_the_threshold_and_refuses_the_absurd() {
    // Con el umbral predeterminado un PDF de unos KB se abre entero (y se puede guardar); el límite superior es del motor.
    let d = DefaultEngine.open_auto(&corpus("hola.pdf"), None).unwrap();
    assert!(d.view_only.is_none());
    let p = Path::new("x.pdf");
    assert!(check_view_only_size(p, VIEW_ONLY_MAX_BYTES).is_ok());
    assert!(matches!(
        check_view_only_size(p, VIEW_ONLY_MAX_BYTES + 1),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn view_only_does_not_keep_the_file_open() {
    // Solo para ver, PDFium lee el archivo por bloques pero no deja ningún manejador abierto entre lecturas: se puede
    // sustituir con un renombrado (en Windows un manejador abierto lo impediría) y el documento avisa de que cambió en
    // lugar de mezclar versiones.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.pdf");
    std::fs::copy(corpus("hola.pdf"), &path).unwrap();
    let d = DefaultEngine.open_for_viewing(&path, None).unwrap();
    assert!(d.page_text(0).unwrap().contains("Hola Pliego"));
    let nuevo = dir.path().join("nuevo.pdf");
    std::fs::write(&nuevo, b"otra cosa, de otro tamano").unwrap();
    std::fs::rename(&nuevo, &path).expect("el archivo abierto para ver debe poder sustituirse");
    // Lo que PDFium ya tenía leído sigue sirviendo; si necesita leer más del archivo nuevo, falla limpio.
    let _ = d.page_text(1);
}
