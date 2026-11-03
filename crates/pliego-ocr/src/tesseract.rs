//! Motor [`OcrEngine`] sobre Tesseract 5 (API C).
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char};
use std::path::PathBuf;
use std::ptr;

use pliego_core::{Error, Result};
use tesseract_sys as sys;

use crate::{GrayImage, OcrEngine, PixelRect, Word, tessdata};

/// Motor Tesseract con uno o varios idiomas (`"spa+eng"`). No es `Sync`; usa un motor por hilo.
pub struct TesseractEngine {
    api: *mut sys::TessBaseAPI,
    langs: String,
    data_dir: PathBuf,
    psm: i32,
    /// Motor auxiliar con el modelo `osd`, creado la primera vez que se pide la orientación.
    osd: Option<Box<TesseractEngine>>,
}

// SAFETY: el manejador de Tesseract pertenece en exclusiva a esta estructura y solo se usa con `&mut self`;
// Tesseract admite mover un manejador entre hilos mientras no se use a la vez desde dos.
unsafe impl Send for TesseractEngine {}

fn cstring(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| Error::InvalidInput("texto con carácter nulo".into()))
}

impl TesseractEngine {
    /// Crea el motor. `data_dir`: carpeta con los `*.traineddata`; si es `None` se busca (ver [`tessdata`]).
    pub fn new(langs: &str, data_dir: Option<PathBuf>) -> Result<Self> {
        let data_dir = match data_dir {
            Some(d) => d,
            None => tessdata::resolve_for(langs)?,
        };
        // SAFETY: crear un manejador no tiene precondiciones.
        let api = unsafe { sys::TessBaseAPICreate() };
        if api.is_null() {
            return Err(Error::Other("no se pudo iniciar Tesseract".into()));
        }
        let engine = Self {
            api,
            langs: langs.to_owned(),
            data_dir,
            psm: sys::PSM_AUTO,
            osd: None,
        };
        engine.init(langs)?;
        Ok(engine)
    }

    fn init(&self, langs: &str) -> Result<()> {
        let dir = cstring(&self.data_dir.to_string_lossy())?;
        let l = cstring(langs)?;
        // SAFETY: `api` es válido y las cadenas viven durante la llamada.
        let rc = unsafe { sys::TessBaseAPIInit3(self.api, dir.as_ptr(), l.as_ptr()) };
        if rc != 0 {
            return Err(Error::Other(format!(
                "Tesseract no pudo cargar los idiomas «{langs}» desde {}",
                self.data_dir.display()
            )));
        }
        Ok(())
    }

    /// Versión de la biblioteca Tesseract enlazada.
    pub fn version() -> String {
        // SAFETY: devuelve una cadena estática terminada en nulo.
        unsafe { CStr::from_ptr(sys::TessVersion()) }
            .to_string_lossy()
            .into_owned()
    }

    pub fn languages(&self) -> &str {
        &self.langs
    }

    /// Fija una variable de Tesseract (p. ej. `preserve_interword_spaces`).
    pub fn set_variable(&mut self, name: &str, value: &str) -> Result<()> {
        let (n, v) = (cstring(name)?, cstring(value)?);
        // SAFETY: manejador y cadenas válidos durante la llamada.
        let ok = unsafe { sys::TessBaseAPISetVariable(self.api, n.as_ptr(), v.as_ptr()) };
        if ok == 0 {
            return Err(Error::InvalidInput(format!("Tesseract no admite la variable {name}")));
        }
        Ok(())
    }

    fn set_image(&mut self, img: &GrayImage, dpi: u32) -> Result<()> {
        let (w, h) = (i32::try_from(img.width), i32::try_from(img.height));
        let (Ok(w), Ok(h)) = (w, h) else {
            return Err(Error::InvalidInput("imagen demasiado grande para OCR".into()));
        };
        if img.data.len() != (img.width as usize) * (img.height as usize) {
            return Err(Error::InvalidInput(
                "el tamaño de la imagen no coincide con sus datos".into(),
            ));
        }
        // SAFETY: `img.data` vive hasta que termine `recognize` (Tesseract no copia, y se llama a
        // `TessBaseAPIClear` antes de salir); las dimensiones se comprobaron arriba.
        unsafe {
            sys::TessBaseAPISetPageSegMode(self.api, self.psm);
            sys::TessBaseAPISetImage(self.api, img.data.as_ptr(), w, h, 1, w);
            sys::TessBaseAPISetSourceResolution(self.api, dpi.clamp(70, 2400) as i32);
        }
        Ok(())
    }
}

impl Drop for TesseractEngine {
    fn drop(&mut self) {
        // SAFETY: `api` se creó en `new` y no se usa después.
        unsafe {
            sys::TessBaseAPIEnd(self.api);
            sys::TessBaseAPIDelete(self.api);
        }
    }
}

/// Lee y libera una cadena devuelta por Tesseract.
///
/// # Safety
/// `p` debe ser nulo o un puntero devuelto por la API de Tesseract que aún no se liberó.
unsafe fn take_text(p: *mut c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: por contrato, `p` es una cadena válida terminada en nulo.
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    // SAFETY: se liberó una sola vez con la función de Tesseract.
    unsafe { sys::TessDeleteText(p) };
    Some(s)
}

impl OcrEngine for TesseractEngine {
    fn recognize(&mut self, img: &GrayImage, dpi: u32) -> Result<Vec<Word>> {
        self.set_image(img, dpi)?;
        let mut words = Vec::new();
        // SAFETY: imagen ya cargada; el iterador pertenece a `api` y se libera antes de `Clear`.
        unsafe {
            if sys::TessBaseAPIRecognize(self.api, ptr::null_mut()) != 0 {
                sys::TessBaseAPIClear(self.api);
                return Err(Error::Other("Tesseract falló al reconocer la página".into()));
            }
            let it = sys::TessBaseAPIGetIterator(self.api);
            if !it.is_null() {
                let page_it = sys::TessResultIteratorGetPageIteratorConst(it);
                loop {
                    let text = take_text(sys::TessResultIteratorGetUTF8Text(it, sys::RIL_WORD));
                    let (mut l, mut t, mut r, mut b) = (0, 0, 0, 0);
                    let has_box =
                        sys::TessPageIteratorBoundingBox(page_it, sys::RIL_WORD, &mut l, &mut t, &mut r, &mut b) != 0;
                    if let (Some(text), true) = (text, has_box) {
                        let text = text.trim().to_owned();
                        if !text.is_empty() {
                            words.push(Word {
                                text,
                                bbox: PixelRect {
                                    x0: l as f32,
                                    y0: t as f32,
                                    x1: r as f32,
                                    y1: b as f32,
                                },
                                confidence: sys::TessResultIteratorConfidence(it, sys::RIL_WORD),
                            });
                        }
                    }
                    if sys::TessResultIteratorNext(it, sys::RIL_WORD) == 0 {
                        break;
                    }
                }
                sys::TessResultIteratorDelete(it);
            }
            sys::TessBaseAPIClear(self.api);
        }
        Ok(words)
    }

    fn detect_orientation(&mut self, img: &GrayImage) -> Result<Option<(u32, f32)>> {
        // La detección necesita el modelo `osd`; se usa un manejador aparte.
        if self.osd.is_none() {
            let Some(dir) = tessdata::find_model_dir("osd") else {
                return Ok(None);
            };
            let mut osd = TesseractEngine::new_raw()?;
            osd.data_dir = dir;
            osd.init("osd")?;
            osd.psm = sys::PSM_OSD_ONLY;
            self.osd = Some(Box::new(osd));
        }
        let osd = self.osd.as_mut().expect("motor osd creado arriba");
        osd.set_image(img, 300)?;
        let (mut deg, mut conf) = (0i32, 0f32);
        let (mut script, mut script_conf): (*const c_char, f32) = (ptr::null(), 0.0);
        // SAFETY: punteros de salida válidos; `script` lo posee Tesseract y no se libera aquí.
        let ok = unsafe {
            sys::TessBaseAPIDetectOrientationScript(osd.api, &mut deg, &mut conf, &mut script, &mut script_conf)
        };
        // SAFETY: manejador válido.
        unsafe { sys::TessBaseAPIClear(osd.api) };
        if ok == 0 {
            return Ok(None);
        }
        // Tesseract devuelve los grados que el texto está girado a favor del reloj; para enderezar hay que
        // girar en sentido contrario.
        Ok(Some((((360 - deg.rem_euclid(360)) % 360) as u32, conf)))
    }
}

impl TesseractEngine {
    fn new_raw() -> Result<Self> {
        // SAFETY: crear un manejador no tiene precondiciones.
        let api = unsafe { sys::TessBaseAPICreate() };
        if api.is_null() {
            return Err(Error::Other("no se pudo iniciar Tesseract".into()));
        }
        Ok(Self {
            api,
            langs: String::new(),
            data_dir: PathBuf::new(),
            psm: sys::PSM_AUTO,
            osd: None,
        })
    }
}
