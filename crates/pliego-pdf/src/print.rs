//! Documento abierto en PDFium **para imprimir** (W45).
//!
//! La impresión no pasa por el dibujo a mapas de bits de [`crate::Page::to_pixmap`]: en Windows PDFium sabe dibujar una
//! página directamente en el contexto de dispositivo (HDC) de una impresora (`FPDF_RenderPage`), de modo que el texto y
//! los vectores salen como órdenes GDI y no como una imagen enorme. Este módulo es la única parte de `pliego-pdf` que
//! toca los manejadores en crudo de PDFium; quien imprime (el crate `pliego-printer`) solo ve [`PrintDocument`].
//!
//! Las páginas se numeran desde 0 y los tamaños son puntos PDF con el giro de la página aplicado.

#![allow(unsafe_code)]

use pdfium_render::prelude::*;
use pliego_core::{Error, Result};

use crate::pdfium::lib;

/// Códigos de `FPDF_GetLastError` que nos interesan.
const ERR_PASSWORD: u64 = 4;

/// Indicadores de `FPDF_RenderPage` (`fpdfview.h`).
pub mod flags {
    /// Dibuja las anotaciones que no son campos de formulario.
    pub const ANNOT: i32 = 0x01;
    /// Texto sin suavizado LCD.
    pub const NO_NATIVE_TEXT: i32 = 0x04;
    /// Dibuja en escala de grises.
    pub const GRAYSCALE: i32 = 0x08;
    /// Modo impresión: respeta el indicador «No imprimir» de las anotaciones y usa las capas «Print».
    pub const PRINTING: i32 = 0x800;
}

/// PDF abierto en PDFium para imprimir. Nada es `Send`: úsese en el hilo que lo abrió.
pub struct PrintDocument {
    handle: FPDF_DOCUMENT,
    /// PDFium conserva el puntero a estos bytes mientras el documento esté abierto: no se tocan ni se mueven.
    _bytes: Vec<u8>,
}

impl PrintDocument {
    /// Abre `bytes` (un PDF completo, ya descifrado o con su `password`). Carga PDFium si hace falta.
    pub fn open(bytes: Vec<u8>, password: Option<&str>) -> Result<Self> {
        let pdfium = lib()?;
        let bindings = pdfium.bindings();
        // SAFETY: `bytes` queda guardado en `Self` hasta que se cierra el documento y su búfer en el montón no se mueve.
        let handle = unsafe { bindings.FPDF_LoadMemDocument64(&bytes, password) };
        if handle.is_null() {
            // SAFETY: sin argumentos; lee el último error del hilo.
            let code = unsafe { bindings.FPDF_GetLastError() } as u64;
            return Err(if code == ERR_PASSWORD {
                Error::Password
            } else {
                Error::Pdf(format!(
                    "PDFium no pudo abrir el documento para imprimir (código {code})"
                ))
            });
        }
        Ok(Self { handle, _bytes: bytes })
    }

    /// Número de páginas.
    pub fn page_count(&self) -> i32 {
        // SAFETY: `handle` es un documento abierto que no se cierra hasta `Drop`.
        unsafe { lib().map(|p| p.bindings().FPDF_GetPageCount(self.handle)).unwrap_or(0) }
    }

    /// Tamaño `(ancho, alto)` de la página en puntos, con el giro aplicado.
    pub fn page_size(&self, index: i32) -> Result<(f32, f32)> {
        let bindings = lib()?.bindings();
        let page = self.load(index)?;
        // SAFETY: `page` es válida hasta el `FPDF_ClosePage` de abajo.
        let size = unsafe { (bindings.FPDF_GetPageWidthF(page), bindings.FPDF_GetPageHeightF(page)) };
        // SAFETY: se cierra la página abierta arriba, una sola vez.
        unsafe { bindings.FPDF_ClosePage(page) };
        Ok(size)
    }

    fn load(&self, index: i32) -> Result<FPDF_PAGE> {
        if index < 0 || index >= self.page_count() {
            return Err(Error::InvalidInput(format!("la página {} no existe", index + 1)));
        }
        let bindings = lib()?.bindings();
        // SAFETY: `handle` es un documento abierto e `index` está dentro del rango.
        let page = unsafe { bindings.FPDF_LoadPage(self.handle, index) };
        if page.is_null() {
            return Err(Error::Pdf(format!("PDFium no pudo cargar la página {}", index + 1)));
        }
        Ok(page)
    }

    /// Dibuja la página `index` en el contexto de dispositivo `hdc` (de una impresora) dentro del rectángulo
    /// `(x, y, ancho, alto)` en unidades del dispositivo. `rotate`: 0, 1 (90° horario), 2 o 3 sobre el giro propio de
    /// la página; con 1 o 3 el rectángulo es el del resultado ya girado. `flags`: véase [`flags`].
    ///
    /// # Safety
    /// `hdc` debe ser un contexto de dispositivo de Windows válido, con una página abierta (`StartPage`), que no se cierre
    /// mientras dura la llamada.
    #[cfg(windows)]
    pub unsafe fn draw_page(
        &self,
        index: i32,
        hdc: *mut core::ffi::c_void,
        rect: (i32, i32, i32, i32),
        rotate: i32,
        flags: i32,
    ) -> Result<()> {
        let bindings = lib()?.bindings();
        let page = self.load(index)?;
        let (x, y, w, h) = rect;
        // SAFETY: `page` es válida; el resto lo garantiza quien llama (véase «Safety»).
        unsafe {
            bindings.FPDF_RenderPage(windows::Win32::Graphics::Gdi::HDC(hdc), page, x, y, w, h, rotate, flags);
            bindings.FPDF_ClosePage(page);
        }
        Ok(())
    }
}

impl Drop for PrintDocument {
    fn drop(&mut self) {
        if let Ok(p) = lib() {
            // SAFETY: se cierra una sola vez el documento que abrió `open`; sus bytes siguen vivos hasta después.
            unsafe { p.bindings().FPDF_CloseDocument(self.handle) };
        }
    }
}
