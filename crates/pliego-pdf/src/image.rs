//! Imágenes: decodificar, incrustar en una página, listar y extraer.

use std::cell::RefCell;
use std::rc::Rc;

use pliego_core::{Error, Result};

use crate::backend::{err, matrix_from, matrix_to, path_str, rect_from, rect_to};
use crate::document::PdfDocument;
use crate::geom::{Matrix, Rect};
use crate::object::Object;
use crate::page::PdfPage;
use crate::pixmap::{Colorspace, Pixmap};

/// Imagen decodificada o a punto de decodificarse. No es `Send`.
#[derive(Clone)]
pub struct Image {
    pub(crate) raw: mupdf::Image,
}

impl Image {
    /// Decodifica PNG, JPEG, JPEG 2000, GIF, TIFF, BMP…
    pub fn from_bytes(bytes: &[u8]) -> Result<Image> {
        mupdf::Image::from_bytes(bytes).map(|raw| Image { raw }).map_err(err)
    }

    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Image> {
        mupdf::Image::from_file(path_str(path.as_ref())?)
            .map(|raw| Image { raw })
            .map_err(err)
    }

    pub fn from_pixmap(pixmap: &Pixmap) -> Result<Image> {
        let p = pixmap.to_backend()?;
        mupdf::Image::from_pixmap(&p).map(|raw| Image { raw }).map_err(err)
    }

    pub fn width(&self) -> u32 {
        self.raw.width()
    }
    pub fn height(&self) -> u32 {
        self.raw.height()
    }
    /// Componentes por píxel (alfa incluido si lo hay).
    pub fn n(&self) -> u8 {
        self.raw.n()
    }
    pub fn bits_per_component(&self) -> u8 {
        self.raw.bits_per_components()
    }
    /// Resolución (ppp horizontal, vertical) declarada.
    pub fn resolution(&self) -> (i32, i32) {
        self.raw.resolution()
    }
    pub fn mask(&self) -> Option<Image> {
        self.raw.mask().map(|raw| Image { raw })
    }

    /// Decodifica a un mapa de bits.
    pub fn to_pixmap(&self) -> Result<Pixmap> {
        let p = self.raw.to_pixmap().map_err(err)?;
        Ok(Pixmap::from_backend(&p))
    }
}

/// Origen de la imagen que se inserta en una página.
pub enum ImageSource<'a> {
    Image(&'a Image),
    Pixmap(&'a Pixmap),
    /// Bytes de un archivo de imagen; `format_hint` es la extensión si se conoce.
    Bytes {
        data: &'a [u8],
        format_hint: Option<&'a str>,
    },
    /// Un objeto de imagen que ya está en el documento.
    ExistingXref(i32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsertImageOptions {
    /// Encima (`true`, por defecto) o debajo del contenido existente.
    pub overlay: bool,
    pub opacity: Option<f32>,
    /// Número de objeto de la capa a la que pertenece.
    pub optional_content: Option<i32>,
}

impl Default for InsertImageOptions {
    fn default() -> Self {
        Self {
            overlay: true,
            opacity: None,
            optional_content: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImagePlacement {
    /// Nombre de recurso de la imagen en la página.
    pub name: String,
    pub xref: i32,
    pub rect: Rect,
    pub contents_xref: i32,
}

/// Imagen de los recursos de una página.
#[derive(Debug, Clone, PartialEq)]
pub struct PageImageInfo {
    pub name: String,
    pub xref: i32,
    pub width: u32,
    pub height: u32,
    pub bits_per_component: Option<i32>,
    pub color_space: Option<String>,
    /// Filtro principal (`DCTDecode`, `JPXDecode`, `FlateDecode`…).
    pub filter: Option<String>,
}

/// Imagen tal como está codificada en el PDF (p. ej. los bytes de un JPEG).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedImage {
    pub xref: i32,
    pub width: u32,
    pub height: u32,
    pub bits_per_component: Option<i32>,
    pub color_space: Option<String>,
    pub filter: Option<String>,
    pub encoded: Vec<u8>,
}

impl PdfDocument {
    /// Imagen codificada del objeto `xref`.
    pub fn extract_image(&self, xref: i32) -> Result<ExtractedImage> {
        let e = self.pdf().extract_image(xref).map_err(err)?;
        Ok(ExtractedImage {
            xref: e.xref,
            width: e.width,
            height: e.height,
            bits_per_component: e.bits_per_component,
            color_space: e.color_space,
            filter: e.filter,
            encoded: e.encoded,
        })
    }

    /// Añade una imagen como XObject y devuelve su referencia.
    pub fn add_image(&mut self, image: &Image) -> Result<Object> {
        self.pdf_mut().add_image(&image.raw).map(Object::wrap).map_err(err)
    }
}

/// Dispositivo que solo anota qué imágenes se dibujan y con qué matriz.
struct Collector(Rc<RefCell<Vec<(Image, Matrix)>>>);

impl mupdf::NativeDevice for Collector {
    fn fill_image(&mut self, img: &mupdf::Image, cmt: mupdf::Matrix, _alpha: f32, _cp: mupdf::ColorParams) {
        self.0.borrow_mut().push((Image { raw: img.clone() }, matrix_from(cmt)));
    }
}

impl PdfPage {
    /// Imágenes de los recursos de la página.
    pub fn images(&self) -> Result<Vec<PageImageInfo>> {
        Ok(self
            .pdf()
            .images()
            .map_err(err)?
            .into_iter()
            .map(|i| PageImageInfo {
                name: i.name,
                xref: i.xref,
                width: i.width,
                height: i.height,
                bits_per_component: i.bits_per_component,
                color_space: i.color_space,
                filter: i.filter,
            })
            .collect())
    }

    /// Inserta una imagen que ocupa `rect` (espacio de usuario de la página).
    pub fn insert_image(
        &mut self,
        doc: &mut PdfDocument,
        rect: Rect,
        source: ImageSource<'_>,
        options: InsertImageOptions,
    ) -> Result<ImagePlacement> {
        use mupdf::pdf::PageImageSource as S;
        let pix;
        let src = match source {
            ImageSource::Image(i) => S::Image(&i.raw),
            ImageSource::Pixmap(p) => {
                pix = p.to_backend()?;
                S::Pixmap(&pix)
            }
            ImageSource::Bytes { data, format_hint } => S::Bytes { data, format_hint },
            ImageSource::ExistingXref(x) => S::ExistingXref(x),
        };
        let oc = match options.optional_content {
            Some(x) => Some(mupdf::pdf::OptionalContentRef::new(x).map_err(err)?),
            None => None,
        };
        let o = mupdf::pdf::InsertImageOptions {
            overlay: options.overlay,
            opacity: options.opacity,
            optional_content: oc,
        };
        let p = self
            .pdf_mut()
            .insert_image(doc.pdf_mut(), rect_to(rect), src, o)
            .map_err(err)?;
        Ok(ImagePlacement {
            name: p.name,
            xref: p.xref,
            rect: rect_from(p.rect),
            contents_xref: p.contents_xref,
        })
    }

    /// Sustituye la imagen del objeto `xref` por otra, en todos los sitios donde esta página la usa.
    pub fn replace_image(
        &mut self,
        doc: &mut PdfDocument,
        xref: i32,
        source: ImageSource<'_>,
    ) -> Result<ImagePlacement> {
        use mupdf::pdf::PageImageSource as S;
        let pix;
        let src = match source {
            ImageSource::Image(i) => S::Image(&i.raw),
            ImageSource::Pixmap(p) => {
                pix = p.to_backend()?;
                S::Pixmap(&pix)
            }
            ImageSource::Bytes { data, format_hint } => S::Bytes { data, format_hint },
            ImageSource::ExistingXref(x) => S::ExistingXref(x),
        };
        let p = self.pdf_mut().replace_image(doc.pdf_mut(), xref, src).map_err(err)?;
        Ok(ImagePlacement {
            name: p.name,
            xref: p.xref,
            rect: rect_from(p.rect),
            contents_xref: p.contents_xref,
        })
    }

    /// Quita la imagen `xref` de los recursos de la página. `true` si estaba.
    pub fn delete_image(&mut self, doc: &mut PdfDocument, xref: i32) -> Result<bool> {
        self.pdf_mut().delete_image(doc.pdf_mut(), xref).map_err(err)
    }

    /// Cada imagen que dibuja el contenido de la página, con la matriz con la que se coloca (en el espacio de
    /// usuario de la página). Sirve para calcular la resolución efectiva.
    pub fn image_placements(&self) -> Result<Vec<(Image, Matrix)>> {
        let out: Rc<RefCell<Vec<(Image, Matrix)>>> = Rc::default();
        let dev = mupdf::Device::from_native(Collector(out.clone())).map_err(err)?;
        self.pdf()
            .run_contents(&dev, &matrix_to(&Matrix::IDENTITY))
            .map_err(err)?;
        drop(dev);
        let v = out.borrow().clone();
        Ok(v)
    }
}

/// Convierte una cadena de espacio de color de imagen en un [`Colorspace`] de salida, si es uno de los básicos.
#[allow(dead_code)]
pub(crate) fn colorspace_for(n: u8) -> Result<Colorspace> {
    match n {
        1 | 2 => Ok(Colorspace::Gray),
        3 | 4 => Ok(Colorspace::Rgb),
        _ => Err(Error::Unsupported(format!("imagen con {n} componentes"))),
    }
}
