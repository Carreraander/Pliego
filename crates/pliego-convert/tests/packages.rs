//! CBZ, EPUB y XPS → PDF (W41). Los paquetes se construyen en la prueba con `zip`; el PDF resultante se lee con PDFium.

use std::io::Write;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use pliego_convert::{ImagesToPdfOptions, TextExportOptions, TextToPdfOptions, cbz_to_pdf, epub_to_pdf, pdf_to_text};
use pliego_core::{JobContext, PageRange};
use pliego_pdf::Document;
use zip::write::SimpleFileOptions;

pub fn zip_file(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    for (name, data) in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

fn png(w: u32, h: u32, color: [u8; 3]) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |_, _| Rgb(color));
    let mut out = Vec::new();
    DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
        .unwrap();
    out
}

fn text_of(pdf: &Path) -> Vec<String> {
    let out = pdf.with_extension("txt");
    pdf_to_text(
        pdf,
        None,
        &PageRange::all(),
        &out,
        &TextExportOptions::default(),
        &JobContext::silent(),
    )
    .unwrap();
    std::fs::read_to_string(out)
        .unwrap()
        .split('\u{c}')
        .map(str::to_owned)
        .collect()
}

fn page_sizes(pdf: &Path) -> Vec<(f32, f32)> {
    let doc = Document::open(pdf).unwrap();
    (0..doc.page_count().unwrap())
        .map(|i| {
            let b = doc.load_page(i).unwrap().bounds().unwrap();
            (b.x1 - b.x0, b.y1 - b.y0)
        })
        .collect()
}

#[test]
fn cbz_pages_follow_natural_order_and_skip_junk() {
    let dir = tempfile::tempdir().unwrap();
    let cbz = dir.path().join("comic.cbz");
    zip_file(
        &cbz,
        &[
            ("cap/p10.png", png(30, 30, [0, 0, 255])),
            ("cap/p2.png", png(20, 40, [0, 255, 0])),
            ("cap/p1.png", png(40, 20, [255, 0, 0])),
            ("__MACOSX/cap/._p1.png", b"basura".to_vec()),
            ("ComicInfo.xml", b"<ComicInfo/>".to_vec()),
        ],
    );
    let pdf = dir.path().join("comic.pdf");
    let opts = ImagesToPdfOptions {
        dpi: 72.0,
        ..Default::default()
    };
    let r = cbz_to_pdf(&cbz, &pdf, &opts, &JobContext::silent()).unwrap();
    assert!(r.summary.contains("3 página"), "{}", r.summary);
    let sizes = page_sizes(&pdf);
    assert_eq!(sizes.len(), 3, "{sizes:?}");
    let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5;
    assert!(near(sizes[0], (40.0, 20.0)), "p1 primero: {sizes:?}");
    assert!(near(sizes[1], (20.0, 40.0)), "p2 segundo: {sizes:?}");
    assert!(near(sizes[2], (30.0, 30.0)), "p10 último: {sizes:?}");
}

#[test]
fn cbz_without_images_or_not_a_zip_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("vacio.cbz");
    zip_file(&empty, &[("notas.txt", b"hola".to_vec())]);
    let out = dir.path().join("o.pdf");
    let e = cbz_to_pdf(&empty, &out, &ImagesToPdfOptions::default(), &JobContext::silent()).unwrap_err();
    assert!(e.to_string().contains("no contiene imágenes"), "{e}");
    let bad = dir.path().join("malo.cbz");
    std::fs::write(&bad, b"esto no es un zip").unwrap();
    let e = cbz_to_pdf(&bad, &out, &ImagesToPdfOptions::default(), &JobContext::silent()).unwrap_err();
    assert!(e.to_string().contains("no es un paquete ZIP"), "{e}");
    assert!(!out.exists());
}

fn epub_entries(extra: Vec<(&'static str, Vec<u8>)>) -> Vec<(&'static str, Vec<u8>)> {
    let mut v: Vec<(&'static str, Vec<u8>)> = vec![
        ("mimetype", b"application/epub+zip".to_vec()),
        (
            "META-INF/container.xml",
            br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#
                .to_vec(),
        ),
        (
            "OEBPS/content.opf",
            br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Libro de prueba</dc:title></metadata>
<manifest>
 <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
 <item id="c1" href="text/cap%201.xhtml" media-type="application/xhtml+xml"/>
 <item id="c2" href="text/cap2.xhtml" media-type="application/xhtml+xml"/>
 <item id="img" href="img/foto.png" media-type="image/png"/>
</manifest>
<spine><itemref idref="nav"/><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#
                .to_vec(),
        ),
        ("OEBPS/nav.xhtml", b"<html><body><nav>INDICE-NO-DEBE-SALIR</nav></body></html>".to_vec()),
        (
            "OEBPS/text/cap 1.xhtml",
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"x\">\n\
<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>NO-SALE-EL-TITULO-DEL-HEAD</title></head>\
<body><h1>Capítulo uno</h1><p>Texto con&nbsp;<b>negrita</b> y &eacute;nfasis.</p>\
<p><img src=\"../img/foto.png\" alt=\"una foto\"/></p><p><img src=\"../img/falta.png\" alt=\"imagen que falta\"/></p></body></html>"
                .as_bytes()
                .to_vec(),
        ),
        (
            "OEBPS/text/cap2.xhtml",
            b"<html><body><h1>Segundo capitulo</h1><p>Final del libro.</p></body></html>".to_vec(),
        ),
    ];
    v.extend(extra);
    v
}

#[test]
fn epub_chapters_images_and_page_breaks() {
    let dir = tempfile::tempdir().unwrap();
    let epub = dir.path().join("libro.epub");
    zip_file(
        &epub,
        &epub_entries(vec![("OEBPS/img/foto.png", png(200, 100, [200, 30, 30]))]),
    );
    let pdf = dir.path().join("libro.pdf");
    let r = epub_to_pdf(&epub, &pdf, &TextToPdfOptions::default(), &JobContext::silent()).unwrap();
    assert!(r.summary.contains("2 capítulo"), "{}", r.summary);
    let pages = text_of(&pdf);
    assert!(pages.len() >= 2, "{pages:?}");
    assert!(
        pages[0].contains("Capítulo uno") && pages[0].contains("negrita"),
        "{:?}",
        pages[0]
    );
    assert!(pages[0].contains("énfasis"), "entidades con nombre: {:?}", pages[0]);
    assert!(
        pages[0].contains("[imagen que falta]"),
        "alt de la que falta: {:?}",
        pages[0]
    );
    assert!(
        !pages[0].contains("[una foto]"),
        "la imagen presente se coloca: {:?}",
        pages[0]
    );
    let all = pages.join("\n");
    assert!(all.contains("Segundo capitulo") && all.contains("Final del libro"));
    assert!(!pages[0].contains("Segundo capitulo"), "cada capítulo en página nueva");
    assert!(!all.contains("INDICE-NO-DEBE-SALIR") && !all.contains("NO-SALE-EL-TITULO-DEL-HEAD"));
    // La imagen está dentro del PDF.
    let doc = Document::open(&pdf).unwrap();
    let with_image = (0..doc.page_count().unwrap())
        .filter(|&i| !doc.load_page(i).unwrap().images().unwrap().is_empty())
        .count();
    assert_eq!(with_image, 1);
    // ...y se dibuja dentro de la página 1: hay píxeles del rojo de la imagen.
    let page = doc.load_page(0).unwrap();
    let pix = page
        .to_pixmap(
            &pliego_pdf::Matrix::new_scale(1.0, 1.0),
            pliego_pdf::Colorspace::Rgb,
            false,
            false,
        )
        .unwrap();
    let reds = pix
        .samples()
        .chunks(3)
        .filter(|p| p.len() == 3 && p[0] > 180 && p[1] < 80 && p[2] < 80)
        .count();
    assert!(
        reds > 2000,
        "la foto debería ocupar una zona visible de la página: {reds} píxeles rojos"
    );
}

#[test]
fn epub_without_the_image_falls_back_to_alt_text() {
    let dir = tempfile::tempdir().unwrap();
    let epub = dir.path().join("libro.epub");
    zip_file(&epub, &epub_entries(vec![]));
    let pdf = dir.path().join("libro.pdf");
    let r = epub_to_pdf(&epub, &pdf, &TextToPdfOptions::default(), &JobContext::silent()).unwrap();
    assert!(r.warnings.iter().any(|w| w.contains("imagen")), "{:?}", r.warnings);
    assert!(text_of(&pdf)[0].contains("[una foto]"));
}

#[test]
fn broken_epub_gives_clear_errors() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("o.pdf");
    let no_container: PathBuf = dir.path().join("a.epub");
    zip_file(&no_container, &[("x.txt", b"hola".to_vec())]);
    let e = epub_to_pdf(&no_container, &out, &TextToPdfOptions::default(), &JobContext::silent()).unwrap_err();
    assert!(e.to_string().contains("container.xml"), "{e}");
    let empty_spine: PathBuf = dir.path().join("b.epub");
    let mut entries = epub_entries(vec![]);
    entries.retain(|(n, _)| *n != "OEBPS/content.opf");
    entries.push((
        "OEBPS/content.opf",
        br#"<package xmlns="http://www.idpf.org/2007/opf"><manifest/><spine/></package>"#.to_vec(),
    ));
    zip_file(&empty_spine, &entries);
    let e = epub_to_pdf(&empty_spine, &out, &TextToPdfOptions::default(), &JobContext::silent()).unwrap_err();
    assert!(e.to_string().contains("spine"), "{e}");
    assert!(!out.exists());
}

// ---------------------------------------------------------------------------------------------------------------------
// XPS

const FONT_GUID: &str = "D1B2C3A4-0102-4304-8506-07080A0B0C0D";

/// Ofusca una fuente como lo hace XPS (la operación es su propia inversa).
fn obfuscate(font: &[u8]) -> Vec<u8> {
    let hex: String = FONT_GUID.chars().filter(|c| *c != '-').collect();
    let key: Vec<u8> = (0..16)
        .map(|i| u8::from_str_radix(&hex[30 - 2 * i..32 - 2 * i], 16).unwrap())
        .collect();
    let mut out = font.to_vec();
    for (i, b) in out.iter_mut().take(32).enumerate() {
        *b ^= key[i % 16];
    }
    out
}

fn xps_entries() -> Vec<(String, Vec<u8>)> {
    let font = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../pliego-pdf/fonts/liberation/LiberationSans-Regular.ttf"),
    )
    .unwrap();
    let ns = "http://schemas.microsoft.com/xps/2005/06";
    let page1 = format!(
        r##"<FixedPage xmlns="{ns}" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" Width="816" Height="1056" xml:lang="es-ES">
<FixedPage.Resources><ResourceDictionary><SolidColorBrush x:Key="rojo" Color="#FFFF0000"/></ResourceDictionary></FixedPage.Resources>
<Path Data="M 100,100 L 300,100 300,200 100,200 Z" Fill="{{StaticResource rojo}}"/>
<Glyphs FontUri="/Resources/Fonts/{FONT_GUID}.odttf" FontRenderingEmSize="32" OriginX="100" OriginY="400" Fill="#FF000000" UnicodeString="Hola XPS"/>
<Glyphs FontUri="/Resources/Fonts/{FONT_GUID}.odttf" FontRenderingEmSize="32" OriginX="100" OriginY="500" Fill="#FF000000" UnicodeString="AB" Indices=",300;,300"/>
<Path Data="M 500,100 L 700,100 700,200 500,200 Z" Fill="#FFFF0000" Opacity="0.5"/>
</FixedPage>"##
    );
    let page2 = format!(
        r##"<FixedPage xmlns="{ns}" Width="816" Height="1056">
<Path Data="M 0,0 L 200,0 200,100 0,100 Z"><Path.Fill><ImageBrush ImageSource="/Resources/Images/azul.png" Viewport="0,0,200,100" ViewportUnits="Absolute" TileMode="None"/></Path.Fill></Path>
<Canvas RenderTransform="1,0,0,1,400,500"><Path Data="M 0,0 L 100,0 100,100 0,100 Z" Fill="#FF00FF00"/></Canvas>
<Path Data="M 50,300 L 450,300" Stroke="#FF000000" StrokeThickness="10"/>
</FixedPage>"##
    );
    vec![
        ("[Content_Types].xml".into(), b"<Types/>".to_vec()),
        (
            "_rels/.rels".into(),
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="R1" Type="http://schemas.microsoft.com/xps/2005/06/fixedrepresentation" Target="/FixedDocumentSequence.fdseq"/></Relationships>"#.to_vec(),
        ),
        (
            "FixedDocumentSequence.fdseq".into(),
            format!(r#"<FixedDocumentSequence xmlns="{ns}"><DocumentReference Source="/Documents/1/FixedDocument.fdoc"/></FixedDocumentSequence>"#).into_bytes(),
        ),
        (
            "Documents/1/FixedDocument.fdoc".into(),
            format!(r#"<FixedDocument xmlns="{ns}"><PageContent Source="Pages/1.fpage"/><PageContent Source="Pages/2.fpage"/></FixedDocument>"#).into_bytes(),
        ),
        ("Documents/1/Pages/1.fpage".into(), page1.into_bytes()),
        ("Documents/1/Pages/2.fpage".into(), page2.into_bytes()),
        (format!("Resources/Fonts/{FONT_GUID}.odttf"), obfuscate(&font)),
        ("Resources/Images/azul.png".into(), png(40, 20, [0, 0, 255])),
    ]
}

fn write_zip(path: &Path, entries: &[(String, Vec<u8>)]) {
    let borrowed: Vec<(&str, Vec<u8>)> = entries.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    zip_file(path, &borrowed);
}

fn render(pdf: &Path, page: i32) -> (usize, usize, Vec<u8>) {
    let doc = Document::open(pdf).unwrap();
    let p = doc.load_page(page).unwrap();
    let pix = p
        .to_pixmap(
            &pliego_pdf::Matrix::new_scale(1.0, 1.0),
            pliego_pdf::Colorspace::Rgb,
            false,
            false,
        )
        .unwrap();
    let (w, h, stride) = (pix.width() as usize, pix.height() as usize, pix.stride());
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        rgb.extend_from_slice(&pix.samples()[y * stride..y * stride + w * 3]);
    }
    (w, h, rgb)
}

fn px(img: &(usize, usize, Vec<u8>), x: usize, y: usize) -> [u8; 3] {
    let i = (y * img.0 + x) * 3;
    [img.2[i], img.2[i + 1], img.2[i + 2]]
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(&b).all(|(x, y)| x.abs_diff(*y) < 40)
}

#[test]
fn xps_pages_paths_text_images_and_transforms() {
    let dir = tempfile::tempdir().unwrap();
    let xps = dir.path().join("doc.xps");
    write_zip(&xps, &xps_entries());
    let pdf = dir.path().join("doc.pdf");
    let r = pliego_convert::xps_to_pdf(&xps, &pdf, &JobContext::silent()).unwrap();
    assert!(r.summary.contains("2 página"), "{}", r.summary);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);

    let sizes = page_sizes(&pdf);
    assert_eq!(sizes.len(), 2);
    assert!(
        (sizes[0].0 - 612.0).abs() < 0.1 && (sizes[0].1 - 792.0).abs() < 0.1,
        "{sizes:?}"
    );

    // Texto con la fuente incrustada (ofuscada), seleccionable.
    let pages = text_of(&pdf);
    assert!(pages[0].contains("Hola XPS"), "{:?}", pages[0]);
    assert!(pages[0].contains('A') && pages[0].contains('B'), "{:?}", pages[0]);

    let p1 = render(&pdf, 0);
    // Rectángulo rojo de 100..300 × 100..200 (unidades XPS) = 75..225 × 75..150 pt.
    assert!(near(px(&p1, 150, 110), [255, 0, 0]), "{:?}", px(&p1, 150, 110));
    assert!(near(px(&p1, 400, 600), [255, 255, 255]));
    // Opacidad 0,5: rojo sobre blanco = rosa. Centro del rectángulo 500..700 × 100..200 → (450, 112) pt.
    let pink = px(&p1, 450, 112);
    assert!(
        pink[0] > 230 && (100..160).contains(&pink[1]) && (100..160).contains(&pink[2]),
        "{pink:?}"
    );
    // El texto dejó tinta oscura en su línea (base en y = 400 → 300 pt).
    let dark = (75..400)
        .flat_map(|x| (270..302).map(move |y| (x, y)))
        .filter(|&(x, y)| px(&p1, x, y)[0] < 100)
        .count();
    assert!(dark > 100, "debería haber texto negro en la línea base: {dark}");

    let p2 = render(&pdf, 1);
    // Imagen azul en 0..200 × 0..100 → 0..150 × 0..75 pt.
    assert!(near(px(&p2, 50, 30), [0, 0, 255]), "{:?}", px(&p2, 50, 30));
    assert!(near(px(&p2, 160, 30), [255, 255, 255]));
    // Lienzo desplazado (400, 500): cuadrado verde en 300..375 × 375..450 pt.
    assert!(near(px(&p2, 330, 400), [0, 255, 0]), "{:?}", px(&p2, 330, 400));
    // Trazo negro grueso en y = 300 → 225 pt.
    assert!(near(px(&p2, 150, 225), [0, 0, 0]), "{:?}", px(&p2, 150, 225));
}

#[test]
fn xps_without_fdseq_uses_page_names_and_reports_approximations() {
    let dir = tempfile::tempdir().unwrap();
    let ns = "http://schemas.microsoft.com/xps/2005/06";
    let grad = format!(
        r##"<FixedPage xmlns="{ns}" Width="100" Height="100"><Path Data="M 0,0 L 100,0 100,100 0,100 Z"><Path.Fill><LinearGradientBrush StartPoint="0,0" EndPoint="1,0"><LinearGradientBrush.GradientStops><GradientStop Color="#FF000000" Offset="0"/><GradientStop Color="#FFFFFFFF" Offset="1"/></LinearGradientBrush.GradientStops></LinearGradientBrush></Path.Fill></Path></FixedPage>"##
    );
    let xps = dir.path().join("sin-indice.xps");
    write_zip(
        &xps,
        &[
            ("Pages/10.fpage".to_string(), grad.clone().into_bytes()),
            ("Pages/2.fpage".to_string(), grad.into_bytes()),
        ],
    );
    let pdf = dir.path().join("o.pdf");
    let r = pliego_convert::xps_to_pdf(&xps, &pdf, &JobContext::silent()).unwrap();
    assert_eq!(page_sizes(&pdf).len(), 2);
    assert!(r.warnings.iter().any(|w| w.contains("degradados")), "{:?}", r.warnings);
    // Gris medio en el centro.
    let img = render(&pdf, 0);
    let c = px(&img, 37, 37);
    assert!((100..160).contains(&c[0]) && c[0] == c[1], "{c:?}");
}

#[test]
fn broken_xps_gives_clear_errors() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("o.pdf");
    let empty = dir.path().join("vacio.xps");
    zip_file(&empty, &[("x.txt", b"hola".to_vec())]);
    let e = pliego_convert::xps_to_pdf(&empty, &out, &JobContext::silent()).unwrap_err();
    assert!(e.to_string().contains("no contiene páginas"), "{e}");
    let bad_page = dir.path().join("mala.xps");
    zip_file(&bad_page, &[("Pages/1.fpage", b"<FixedPage><Path".to_vec())]);
    let e = pliego_convert::xps_to_pdf(&bad_page, &out, &JobContext::silent()).unwrap_err();
    assert!(e.to_string().contains("XML válido"), "{e}");
    assert!(!out.exists());
}
