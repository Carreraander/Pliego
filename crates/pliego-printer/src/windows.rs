//! Impresoras de Windows: lista, capacidades y envío de las páginas con PDFium al contexto de dispositivo (GDI).
//!
//! Solo este módulo habla con `winspool` y `gdi32`. Cada bloque `unsafe` lleva su `// SAFETY:`.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::path::Path;
use std::ptr::{null, null_mut};

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::print::{PrintDocument, flags};
use windows_sys::Win32::Graphics::Gdi::{
    CreateDCW, CreateICW, CreatePen, DEVMODEW, DM_COLLATE, DM_COLOR, DM_COPIES, DM_DUPLEX, DM_IN_BUFFER,
    DM_ORIENTATION, DM_OUT_BUFFER, DM_PAPERSIZE, DMCOLOR_MONOCHROME, DeleteDC, DeleteObject, GetDeviceCaps,
    GetStockObject, HDC, HORZRES, LOGPIXELSX, LOGPIXELSY, NULL_BRUSH, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY,
    PHYSICALWIDTH, PS_SOLID, Rectangle, SelectObject, VERTRES,
};
use windows_sys::Win32::Graphics::Printing::{
    ClosePrinter, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW, PRINTER_ENUM_CONNECTIONS,
    PRINTER_ENUM_LOCAL, PRINTER_HANDLE, PRINTER_INFO_4W,
};
use windows_sys::Win32::Storage::Xps::{
    AbortDoc, DC_COLLATE, DC_COLORDEVICE, DC_COPIES, DC_DUPLEX, DC_PAPERNAMES, DC_PAPERS, DC_PAPERSIZE, DOCINFOW,
    DeviceCapabilitiesW, EndDoc, EndPage, StartDocW, StartPage,
};

use crate::devices::{Paper, PrinterCaps, PrinterEntry};
use crate::job::{PrintTarget, run_job};
use crate::layout::{CopiesPlan, SheetMetrics, SheetPlan, choose_landscape, plan_copies, plan_copies_for};
use crate::options::{Duplex, PrintOptions};
use crate::prepare::prepare;

const DMORIENT_PORTRAIT: i16 = 1;
const DMORIENT_LANDSCAPE: i16 = 2;
const DMCOLOR_COLOR: i16 = 2;
const DM_PAPERLENGTH: u32 = 0x4;
const DM_PAPERWIDTH: u32 = 0x8;
const DM_FORMNAME: u32 = 0x1_0000;
const DMDUP_SIMPLEX: i16 = 1;
const DMDUP_VERTICAL: i16 = 2;
const DMDUP_HORIZONTAL: i16 = 3;

/// Cadena UTF-16 terminada en cero para la API de Windows.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Texto de un búfer UTF-16 hasta el primer cero.
fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Nombre de la impresora predeterminada.
pub fn default_printer() -> Option<String> {
    let mut len = 0u32;
    // SAFETY: con búfer nulo y longitud 0 la función solo devuelve en `len` el tamaño necesario.
    unsafe { GetDefaultPrinterW(null_mut(), &mut len) };
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    // SAFETY: `buf` tiene `len` elementos, que es lo que se declara.
    let ok = unsafe { GetDefaultPrinterW(buf.as_mut_ptr(), &mut len) };
    (ok != 0).then(|| from_wide(&buf))
}

/// Impresoras instaladas (locales y de red conectadas), la predeterminada marcada.
pub fn list_printers() -> Result<Vec<PrinterEntry>> {
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let (mut needed, mut returned) = (0u32, 0u32);
    // SAFETY: primera llamada de tamaño: sin búfer, devuelve en `needed` los bytes que hacen falta.
    unsafe { EnumPrintersW(flags, null(), 4, null_mut(), 0, &mut needed, &mut returned) };
    if needed == 0 {
        return Ok(Vec::new());
    }
    // `u64` para que el búfer quede alineado a 8 bytes, como pide PRINTER_INFO_4W.
    let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
    // SAFETY: `buf` ofrece al menos `needed` bytes.
    let ok = unsafe {
        EnumPrintersW(
            flags,
            null(),
            4,
            buf.as_mut_ptr().cast::<u8>(),
            needed,
            &mut needed,
            &mut returned,
        )
    };
    if ok == 0 {
        return Err(Error::Other("Windows no dio la lista de impresoras".into()));
    }
    let default = default_printer();
    let infos = buf.as_ptr().cast::<PRINTER_INFO_4W>();
    let mut out = Vec::new();
    for i in 0..returned as usize {
        // SAFETY: Windows escribió `returned` estructuras seguidas al principio del búfer, y los nombres apuntan dentro de él.
        let name_ptr = unsafe { (*infos.add(i)).pPrinterName };
        if name_ptr.is_null() {
            continue;
        }
        // SAFETY: cadena UTF-16 terminada en cero dentro del búfer.
        let name = unsafe {
            let mut n = 0;
            while *name_ptr.add(n) != 0 {
                n += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(name_ptr, n))
        };
        out.push(PrinterEntry {
            is_default: default.as_deref() == Some(name.as_str()),
            name,
        });
    }
    out.sort_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

/// `DeviceCapabilitiesW` con `output` nulo: cuántos elementos tiene la capacidad (o su valor, si es un número).
fn capability_count(name: &[u16], cap: u16) -> i32 {
    // SAFETY: `name` termina en cero; sin búfer de salida ni DEVMODE.
    unsafe { DeviceCapabilitiesW(name.as_ptr(), null(), cap, null_mut(), null()) }
}

/// Qué sabe hacer la impresora `name`.
pub fn capabilities(name: &str) -> Result<PrinterCaps> {
    let wname = wide(name);
    let papers_n = capability_count(&wname, DC_PAPERS);
    if papers_n < 0 {
        return Err(Error::Other(format!("la impresora «{name}» no responde")));
    }
    let n = papers_n as usize;
    let mut papers = Vec::with_capacity(n);
    if n > 0 {
        let mut ids = vec![0u16; n];
        let mut names = vec![0u16; n * 64];
        let mut sizes = vec![0i32; n * 2];
        // SAFETY: cada búfer tiene espacio para `n` elementos del tamaño que documenta cada capacidad
        // (WORD, 64 caracteres por nombre, POINT = 2 enteros de 32 bits).
        let (a, b, c) = unsafe {
            (
                DeviceCapabilitiesW(wname.as_ptr(), null(), DC_PAPERS, ids.as_mut_ptr(), null()),
                DeviceCapabilitiesW(wname.as_ptr(), null(), DC_PAPERNAMES, names.as_mut_ptr(), null()),
                DeviceCapabilitiesW(
                    wname.as_ptr(),
                    null(),
                    DC_PAPERSIZE,
                    sizes.as_mut_ptr().cast::<u16>(),
                    null(),
                ),
            )
        };
        let got = (a.min(b).min(c)).max(0) as usize;
        for i in 0..got.min(n) {
            papers.push(Paper {
                id: ids[i] as i16,
                name: from_wide(&names[i * 64..(i + 1) * 64]),
                // Décimas de milímetro.
                width_mm: sizes[i * 2] as f32 / 10.0,
                height_mm: sizes[i * 2 + 1] as f32 / 10.0,
            });
        }
    }
    let default_paper = match default_devmode(name) {
        Ok(dm) => {
            // SAFETY: `dm` contiene un DEVMODEW completo que acaba de rellenar el controlador.
            let d = unsafe { &*dm.as_ptr().cast::<DEVMODEW>() };
            // SAFETY: es la variante de impresora de la unión (la otra es de pantallas).
            let paper = unsafe { d.Anonymous1.Anonymous1.dmPaperSize };
            (paper > 0).then_some(paper)
        }
        Err(_) => None,
    };
    Ok(PrinterCaps {
        name: name.to_owned(),
        color: capability_count(&wname, DC_COLORDEVICE) > 0,
        duplex: capability_count(&wname, DC_DUPLEX) > 0,
        max_copies: capability_count(&wname, DC_COPIES).max(1) as u32,
        collate: capability_count(&wname, DC_COLLATE) > 0,
        papers,
        default_paper,
    })
}

/// Un DEVMODE completo (con los datos propios del controlador) en un búfer alineado.
type DevMode = Vec<u64>;

struct PrinterHandle(PRINTER_HANDLE);

impl PrinterHandle {
    fn open(name: &[u16]) -> Result<Self> {
        let mut h = PRINTER_HANDLE::default();
        // SAFETY: `name` termina en cero y `h` es donde Windows deja el manejador.
        let ok = unsafe { OpenPrinterW(name.as_ptr(), &mut h, null()) };
        if ok == 0 {
            return Err(Error::Other(format!(
                "no se pudo abrir la impresora «{}»",
                from_wide(name)
            )));
        }
        Ok(Self(h))
    }
}

impl Drop for PrinterHandle {
    fn drop(&mut self) {
        // SAFETY: manejador abierto con `OpenPrinterW`, que se cierra una sola vez.
        unsafe { ClosePrinter(self.0) };
    }
}

/// Los ajustes por defecto del controlador de `name`.
fn default_devmode(name: &str) -> Result<DevMode> {
    let wname = wide(name);
    let printer = PrinterHandle::open(&wname)?;
    // SAFETY: sin búfer de salida devuelve el tamaño necesario del DEVMODE.
    let size = unsafe { DocumentPropertiesW(null_mut(), printer.0, wname.as_ptr(), null_mut(), null(), 0) };
    if size <= 0 {
        return Err(Error::Other(format!("la impresora «{name}» no da sus ajustes")));
    }
    let mut buf: DevMode = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: `buf` ofrece `size` bytes como ha pedido la llamada anterior.
    let got = unsafe {
        DocumentPropertiesW(
            null_mut(),
            printer.0,
            wname.as_ptr(),
            buf.as_mut_ptr().cast::<DEVMODEW>(),
            null(),
            DM_OUT_BUFFER,
        )
    };
    if got < 0 {
        return Err(Error::Other(format!("la impresora «{name}» no da sus ajustes")));
    }
    Ok(buf)
}

/// Ajustes del trabajo: los del controlador con la orientación, el papel, las copias, las dos caras y el color elegidos.
fn job_devmode(
    name: &str,
    options: &PrintOptions,
    landscape: bool,
    copies: &CopiesPlan,
    caps: &PrinterCaps,
) -> Result<DevMode> {
    let mut dm = default_devmode(name)?;
    {
        // SAFETY: `dm` contiene un DEVMODEW completo (lo rellenó `default_devmode`).
        let d = unsafe { &mut *dm.as_mut_ptr().cast::<DEVMODEW>() };
        // SAFETY: variante de impresora de la unión.
        let p = unsafe { &mut d.Anonymous1.Anonymous1 };
        p.dmOrientation = if landscape {
            DMORIENT_LANDSCAPE
        } else {
            DMORIENT_PORTRAIT
        };
        d.dmFields |= DM_ORIENTATION;
        if let Some(paper) = options.paper {
            p.dmPaperSize = paper;
            // Que no mande un tamaño personalizado o un nombre de formulario anterior.
            d.dmFields &= !(DM_PAPERLENGTH | DM_PAPERWIDTH | DM_FORMNAME);
            d.dmFields |= DM_PAPERSIZE;
        }
        if copies.device_copies > 1 {
            p.dmCopies = copies.device_copies.min(i16::MAX as u32) as i16;
            d.dmFields |= DM_COPIES;
            if copies.device_collate {
                d.dmCollate = 1;
                d.dmFields |= DM_COLLATE;
            }
        }
        if caps.duplex {
            d.dmDuplex = match options.effective_duplex() {
                Duplex::Off => DMDUP_SIMPLEX,
                Duplex::LongEdge => DMDUP_VERTICAL,
                Duplex::ShortEdge => DMDUP_HORIZONTAL,
            };
            d.dmFields |= DM_DUPLEX;
        }
        if caps.color {
            d.dmColor = if options.grayscale {
                DMCOLOR_MONOCHROME
            } else {
                DMCOLOR_COLOR
            };
            d.dmFields |= DM_COLOR;
        }
    }
    // El controlador valida y completa lo pedido (fusiona la entrada con sus propios ajustes).
    let wname = wide(name);
    let printer = PrinterHandle::open(&wname)?;
    let mut merged: DevMode = dm.clone();
    // SAFETY: entrada y salida son búferes del mismo tamaño que el DEVMODE completo.
    let ok = unsafe {
        DocumentPropertiesW(
            null_mut(),
            printer.0,
            wname.as_ptr(),
            merged.as_mut_ptr().cast::<DEVMODEW>(),
            dm.as_ptr().cast::<DEVMODEW>(),
            DM_IN_BUFFER | DM_OUT_BUFFER,
        )
    };
    Ok(if ok >= 0 { merged } else { dm })
}

/// Medidas del dispositivo `hdc`.
fn metrics_of(hdc: HDC) -> SheetMetrics {
    // SAFETY: `hdc` es un contexto válido mientras dure la llamada.
    let get = |index: u32| f64::from(unsafe { GetDeviceCaps(hdc, index as i32) });
    SheetMetrics {
        dpi_x: get(LOGPIXELSX).max(1.0),
        dpi_y: get(LOGPIXELSY).max(1.0),
        phys_w: get(PHYSICALWIDTH),
        phys_h: get(PHYSICALHEIGHT),
        off_x: get(PHYSICALOFFSETX),
        off_y: get(PHYSICALOFFSETY),
        area_w: get(HORZRES),
        area_h: get(VERTRES),
    }
}

/// Zona imprimible de la hoja en **puntos** (para la vista previa del diálogo): papel y orientación dados.
pub fn sheet_in_points(name: &str, paper: Option<i16>, landscape: bool) -> Result<SheetMetrics> {
    let caps = capabilities(name)?;
    let options = PrintOptions {
        paper,
        ..PrintOptions::default()
    };
    let copies = plan_copies(1, true, 1, false);
    let dm = job_devmode(name, &options, landscape, &copies, &caps)?;
    let wname = wide(name);
    // SAFETY: contexto de información (no imprime) con el DEVMODE completo del controlador.
    let ic = unsafe { CreateICW(null(), wname.as_ptr(), null(), dm.as_ptr().cast::<DEVMODEW>()) };
    if ic.is_null() {
        return Err(Error::Other(format!("no se pudo consultar la impresora «{name}»")));
    }
    let m = metrics_of(ic);
    // SAFETY: contexto creado arriba, que se borra una vez.
    unsafe { DeleteDC(ic) };
    let (kx, ky) = (72.0 / m.dpi_x, 72.0 / m.dpi_y);
    Ok(SheetMetrics {
        dpi_x: 72.0,
        dpi_y: 72.0,
        phys_w: m.phys_w * kx,
        phys_h: m.phys_h * ky,
        off_x: m.off_x * kx,
        off_y: m.off_y * ky,
        area_w: m.area_w * kx,
        area_h: m.area_h * ky,
    })
}

/// Contexto de impresora más el documento PDF que se dibuja en él.
struct GdiTarget<'a> {
    hdc: HDC,
    doc: &'a PrintDocument,
    flags: i32,
    output: Option<Vec<u16>>,
    document_open: bool,
}

impl Drop for GdiTarget<'_> {
    fn drop(&mut self) {
        // SAFETY: contexto creado con `CreateDCW`, que se borra una sola vez (al terminar o fallar el trabajo).
        unsafe { DeleteDC(self.hdc) };
    }
}

impl GdiTarget<'_> {
    /// Marco fino (media punta de 72 ppp, al menos un píxel) alrededor del rectángulo dado.
    fn frame(&self, x: i32, y: i32, w: i32, h: i32) {
        let dpi = f64::from(unsafe { GetDeviceCaps(self.hdc, LOGPIXELSX as i32) }).max(1.0);
        let width = ((dpi / 144.0).round() as i32).max(1);
        // SAFETY: lápiz propio que se selecciona, se usa y se devuelve al contexto antes de borrarlo; el pincel nulo es
        // un objeto de serie que no se borra.
        unsafe {
            let pen = CreatePen(PS_SOLID, width, 0);
            let old_pen = SelectObject(self.hdc, pen);
            let old_brush = SelectObject(self.hdc, GetStockObject(NULL_BRUSH));
            Rectangle(self.hdc, x, y, x + w, y + h);
            SelectObject(self.hdc, old_brush);
            SelectObject(self.hdc, old_pen);
            DeleteObject(pen);
        }
    }
}

impl PrintTarget for GdiTarget<'_> {
    fn metrics(&self) -> SheetMetrics {
        metrics_of(self.hdc)
    }

    fn start_document(&mut self, title: &str) -> Result<()> {
        let name = wide(title);
        let info = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: name.as_ptr(),
            lpszOutput: self.output.as_ref().map_or(null(), |o| o.as_ptr()),
            lpszDatatype: null(),
            fwType: 0,
        };
        // SAFETY: `info` y las cadenas a las que apunta viven durante la llamada.
        let job = unsafe { StartDocW(self.hdc, &info) };
        if job <= 0 {
            return Err(Error::Other(
                "la impresora no aceptó el documento (comprueba que está encendida y sin errores)".into(),
            ));
        }
        self.document_open = true;
        Ok(())
    }

    fn print_sheet(&mut self, sheet: &SheetPlan) -> Result<()> {
        // SAFETY: contexto válido con un documento abierto.
        if unsafe { StartPage(self.hdc) } <= 0 {
            return Err(Error::Other("la impresora no aceptó la hoja".into()));
        }
        let mut drawn = Ok(());
        for slot in &sheet.slots {
            let p = &slot.placement;
            // SAFETY: `hdc` es un contexto de impresora válido con una página abierta (StartPage) hasta el EndPage de abajo.
            drawn = unsafe {
                self.doc.draw_page(
                    slot.page as i32,
                    self.hdc.cast::<c_void>(),
                    (p.x, p.y, p.w, p.h),
                    p.rotate,
                    self.flags,
                )
            };
            if drawn.is_err() {
                break;
            }
            if sheet.borders {
                self.frame(p.x, p.y, p.w, p.h);
            }
        }
        // SAFETY: cierra la hoja abierta arriba.
        let ended = unsafe { EndPage(self.hdc) };
        drawn?;
        if ended <= 0 {
            return Err(Error::Other("la impresora rechazó la hoja al terminarla".into()));
        }
        Ok(())
    }

    fn end_document(&mut self) -> Result<()> {
        self.document_open = false;
        // SAFETY: contexto válido con un documento abierto.
        if unsafe { EndDoc(self.hdc) } <= 0 {
            return Err(Error::Other("la impresora no pudo terminar el documento".into()));
        }
        Ok(())
    }

    fn abort_document(&mut self) {
        if self.document_open {
            self.document_open = false;
            // SAFETY: contexto válido con un documento abierto.
            unsafe { AbortDoc(self.hdc) };
        }
    }
}

/// Imprime `path` con las opciones dadas.
pub fn print_file(path: &Path, password: Option<&str>, options: &PrintOptions, ctx: &JobContext) -> Result<Report> {
    options.validate()?;
    ctx.step(0, 1, "Preparando el documento")?;
    let prepared = prepare(path, password)?;
    let doc = PrintDocument::open(prepared.bytes, prepared.password.as_deref())?;
    let count = doc.page_count().max(0) as u32;
    let first_page = options.base_selection(count)?[0];
    let sizes = (0..count as i32)
        .map(|i| doc.page_size(i))
        .collect::<Result<Vec<_>>>()?;

    let name = match options.printer.clone().or_else(default_printer) {
        Some(n) => n,
        None => {
            return Err(Error::Unsupported(
                "no hay ninguna impresora instalada: añade una en la configuración de Windows".into(),
            ));
        }
    };
    let caps = capabilities(&name)?;
    let mut warnings = Vec::new();
    let duplex = options.effective_duplex() != Duplex::Off;
    if duplex && !caps.duplex {
        warnings.push(format!("«{name}» no imprime a doble cara: se imprimió a una cara"));
    }
    let copies = plan_copies_for(
        options.copies,
        options.collate,
        caps.max_copies,
        caps.collate,
        duplex && caps.duplex,
    );
    let landscape = choose_landscape(options, sizes[first_page as usize]);
    let devmode = job_devmode(&name, options, landscape, &copies, &caps)?;

    let wname = wide(&name);
    // SAFETY: `wname` termina en cero y el DEVMODE es el completo que dejó el controlador.
    let hdc = unsafe { CreateDCW(null(), wname.as_ptr(), null(), devmode.as_ptr().cast::<DEVMODEW>()) };
    if hdc.is_null() {
        return Err(Error::Other(format!("no se pudo abrir la impresora «{name}»")));
    }
    let mut flag_bits = flags::PRINTING;
    if options.comments {
        flag_bits |= flags::ANNOT;
    }
    if options.grayscale {
        flag_bits |= flags::GRAYSCALE;
    }
    let mut target = GdiTarget {
        hdc,
        doc: &doc,
        flags: flag_bits,
        output: options.output.as_ref().map(|o| wide(&o.to_string_lossy())),
        document_open: false,
    };
    let title = path
        .file_name()
        .map_or_else(|| "Pliego".to_owned(), |n| n.to_string_lossy().into_owned());
    let sent = run_job(&sizes, options, copies, &mut target, &title, ctx)?;
    let summary = if copies.device_copies > 1 {
        format!(
            "Se enviaron {} hoja(s) × {} copias a «{name}»",
            sent.distinct_sheets, copies.device_copies
        )
    } else {
        format!("Se enviaron {} hoja(s) a «{name}»", sent.sheets_sent)
    };
    Ok(Report {
        outputs: options.output.iter().cloned().collect(),
        warnings,
        summary,
    })
}
