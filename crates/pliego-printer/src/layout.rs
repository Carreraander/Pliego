//! Maquetación de una página PDF en la hoja. Son cuentas puras (sin Windows, sin PDFium): las usan el envío a la
//! impresora y la vista previa del diálogo, de modo que lo que se ve es lo que sale.

use serde::{Deserialize, Serialize};

use crate::options::{Orientation, PrintOptions, ScaleMode};

/// Medidas de la hoja en unidades del dispositivo (píxeles de la impresora; en la vista previa, puntos con 72 ppp).
///
/// El origen de coordenadas del contexto de la impresora es la esquina superior izquierda de la **zona imprimible**,
/// que queda a `(off_x, off_y)` de la esquina de la hoja física.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetMetrics {
    pub dpi_x: f64,
    pub dpi_y: f64,
    /// Tamaño de la hoja física.
    pub phys_w: f64,
    pub phys_h: f64,
    /// Distancia de la esquina de la hoja a la de la zona imprimible.
    pub off_x: f64,
    pub off_y: f64,
    /// Tamaño de la zona imprimible.
    pub area_w: f64,
    pub area_h: f64,
}

impl SheetMetrics {
    /// Hoja sin márgenes no imprimibles, en puntos (para pruebas y vista previa).
    pub fn plain(width_pt: f64, height_pt: f64) -> Self {
        Self {
            dpi_x: 72.0,
            dpi_y: 72.0,
            phys_w: width_pt,
            phys_h: height_pt,
            off_x: 0.0,
            off_y: 0.0,
            area_w: width_pt,
            area_h: height_pt,
        }
    }

    /// `true` si la hoja es apaisada.
    pub fn is_landscape(&self) -> bool {
        self.phys_w * self.dpi_y > self.phys_h * self.dpi_x
    }
}

/// Dónde y cómo se dibuja una página: rectángulo en el sistema del contexto de la impresora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    /// 0 o 1 (90° en sentido horario), el `rotate` de `FPDF_RenderPage`.
    pub rotate: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Coloca una página de `page_w × page_h` puntos en la hoja según las opciones.
pub fn place(page_w: f32, page_h: f32, sheet: &SheetMetrics, options: &PrintOptions) -> Placement {
    let (pw, ph) = (f64::from(page_w).max(1.0), f64::from(page_h).max(1.0));
    // Tamaño de la página en píxeles del dispositivo, sin girar y girada.
    let plain = (pw * sheet.dpi_x / 72.0, ph * sheet.dpi_y / 72.0);
    let turned = (ph * sheet.dpi_x / 72.0, pw * sheet.dpi_y / 72.0);
    let fit = |(w, h): (f64, f64)| (sheet.area_w / w).min(sheet.area_h / h);
    let fits_mode = matches!(options.scale, ScaleMode::Fit | ScaleMode::Shrink);
    let rotate = options.auto_rotate && fits_mode && fit(turned) > fit(plain) * 1.0001;
    let (w, h) = if rotate { turned } else { plain };
    let scale = match options.scale {
        ScaleMode::Fit => fit((w, h)),
        ScaleMode::Shrink => fit((w, h)).min(1.0),
        ScaleMode::Actual => 1.0,
        ScaleMode::Custom => f64::from(options.scale_percent.clamp(10, 400)) / 100.0,
    };
    let (dw, dh) = (w * scale, h * scale);
    let (x, y) = if fits_mode {
        // Centrada en la zona imprimible.
        ((sheet.area_w - dw) / 2.0, (sheet.area_h - dh) / 2.0)
    } else {
        // Tamaño real o porcentaje: centrada en la hoja (una página A4 sobre una hoja A4 cae justo encima), y lo que
        // sobresale de la zona imprimible lo recorta la impresora.
        (
            (sheet.phys_w - dw) / 2.0 - sheet.off_x,
            (sheet.phys_h - dh) / 2.0 - sheet.off_y,
        )
    };
    Placement {
        rotate: i32::from(rotate),
        x: x.round() as i32,
        y: y.round() as i32,
        w: (dw.round() as i32).max(1),
        h: (dh.round() as i32).max(1),
    }
}

/// Orientación de la hoja para el trabajo: la elegida, o la de la primera página que se imprime.
/// Devuelve `true` si es apaisada.
pub fn choose_landscape(options: &PrintOptions, first_page: (f32, f32)) -> bool {
    match options.orientation {
        Orientation::Portrait => false,
        Orientation::Landscape => true,
        Orientation::Auto => first_page.0 > first_page.1,
    }
}

/// Cómo se reparten las copias entre la impresora y Pliego.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopiesPlan {
    /// Copias que pide Pliego al controlador (`dmCopies`).
    pub device_copies: u32,
    /// Intercalado que pide al controlador (`dmCollate`).
    pub device_collate: bool,
    /// Veces que Pliego repite el juego completo (copias intercaladas que el controlador no hace).
    pub set_repeats: u32,
    /// Veces que Pliego repite cada página seguidas (copias sin intercalar que el controlador no hace).
    pub page_repeats: u32,
}

/// Decide cómo hacer `requested` copias. `max_copies`: lo que declara el controlador (`DC_COPIES`);
/// `can_collate`: si sabe intercalar (`DC_COLLATE`). Siempre que puede deja las copias al controlador, que es lo que
/// mantiene bien las hojas a dos caras; si no, las repite Pliego.
pub fn plan_copies(requested: u32, collate: bool, max_copies: u32, can_collate: bool) -> CopiesPlan {
    let requested = requested.max(1);
    if requested == 1 {
        return CopiesPlan {
            device_copies: 1,
            device_collate: false,
            set_repeats: 1,
            page_repeats: 1,
        };
    }
    if max_copies >= requested && (!collate || can_collate) {
        return CopiesPlan {
            device_copies: requested,
            device_collate: collate,
            set_repeats: 1,
            page_repeats: 1,
        };
    }
    CopiesPlan {
        device_copies: 1,
        device_collate: false,
        set_repeats: if collate { requested } else { 1 },
        page_repeats: if collate { 1 } else { requested },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: (f32, f32) = (595.0, 842.0);

    fn opts(scale: ScaleMode) -> PrintOptions {
        PrintOptions {
            scale,
            ..PrintOptions::default()
        }
    }

    #[test]
    fn fit_a4_on_a4_with_margins_is_centered_and_proportional() {
        // 600 ppp, hoja A4, 4 mm de margen no imprimible por lado.
        let dpi = 600.0;
        let mm = dpi / 25.4;
        let sheet = SheetMetrics {
            dpi_x: dpi,
            dpi_y: dpi,
            phys_w: 210.0 * mm,
            phys_h: 297.0 * mm,
            off_x: 4.0 * mm,
            off_y: 4.0 * mm,
            area_w: 202.0 * mm,
            area_h: 289.0 * mm,
        };
        let p = place(A4.0, A4.1, &sheet, &opts(ScaleMode::Fit));
        assert_eq!(p.rotate, 0);
        // Cabe entera y centrada dentro de la zona imprimible.
        assert!(p.x >= 0 && p.y >= 0);
        assert!(f64::from(p.x + p.w) <= sheet.area_w + 1.0 && f64::from(p.y + p.h) <= sheet.area_h + 1.0);
        let ratio = f64::from(p.w) / f64::from(p.h);
        assert!((ratio - 595.0 / 842.0).abs() < 0.001);
        assert!((f64::from(p.x) - (sheet.area_w - f64::from(p.w)) / 2.0).abs() <= 1.0);
    }

    #[test]
    fn actual_size_on_same_paper_lands_on_the_sheet_origin() {
        let sheet = SheetMetrics {
            off_x: 20.0,
            off_y: 30.0,
            area_w: 555.0,
            area_h: 782.0,
            ..SheetMetrics::plain(595.0, 842.0)
        };
        let p = place(595.0, 842.0, &sheet, &opts(ScaleMode::Actual));
        // El origen del contexto está en la zona imprimible: la hoja empieza en (-off_x, -off_y).
        assert_eq!((p.x, p.y, p.w, p.h), (-20, -30, 595, 842));
    }

    #[test]
    fn shrink_never_enlarges_but_reduces_what_does_not_fit() {
        let sheet = SheetMetrics::plain(595.0, 842.0);
        let small = place(200.0, 300.0, &sheet, &opts(ScaleMode::Shrink));
        assert_eq!((small.w, small.h), (200, 300));
        let big = place(1190.0, 1684.0, &sheet, &opts(ScaleMode::Shrink));
        assert_eq!((big.w, big.h), (595, 842));
        let fit_up = place(200.0, 300.0, &sheet, &opts(ScaleMode::Fit));
        assert!(fit_up.w > 200);
    }

    #[test]
    fn custom_percent_scales_around_the_sheet_center() {
        let sheet = SheetMetrics::plain(600.0, 800.0);
        let o = PrintOptions {
            scale: ScaleMode::Custom,
            scale_percent: 50,
            ..PrintOptions::default()
        };
        let p = place(600.0, 800.0, &sheet, &o);
        assert_eq!((p.x, p.y, p.w, p.h), (150, 200, 300, 400));
    }

    #[test]
    fn landscape_page_rotates_on_portrait_sheet_only_when_it_helps() {
        let sheet = SheetMetrics::plain(595.0, 842.0);
        let rotated = place(842.0, 595.0, &sheet, &opts(ScaleMode::Fit));
        assert_eq!(rotated.rotate, 1);
        assert_eq!((rotated.w, rotated.h), (595, 842));
        let off = PrintOptions {
            auto_rotate: false,
            ..opts(ScaleMode::Fit)
        };
        let plain = place(842.0, 595.0, &sheet, &off);
        assert_eq!(plain.rotate, 0);
        assert!(plain.w <= 595);
        // Un cuadrado no gana nada girándolo.
        assert_eq!(place(500.0, 500.0, &sheet, &opts(ScaleMode::Fit)).rotate, 0);
        // Con tamaño real nunca gira.
        assert_eq!(place(842.0, 595.0, &sheet, &opts(ScaleMode::Actual)).rotate, 0);
    }

    #[test]
    fn orientation_choice() {
        let auto = PrintOptions::default();
        assert!(!choose_landscape(&auto, (595.0, 842.0)));
        assert!(choose_landscape(&auto, (842.0, 595.0)));
        let forced = PrintOptions {
            orientation: Orientation::Landscape,
            ..PrintOptions::default()
        };
        assert!(choose_landscape(&forced, (595.0, 842.0)));
        let portrait = PrintOptions {
            orientation: Orientation::Portrait,
            ..PrintOptions::default()
        };
        assert!(!choose_landscape(&portrait, (842.0, 595.0)));
    }

    #[test]
    fn copies_prefer_the_driver_and_fall_back_to_software() {
        let driver = plan_copies(3, true, 99, true);
        assert_eq!((driver.device_copies, driver.device_collate), (3, true));
        assert_eq!((driver.set_repeats, driver.page_repeats), (1, 1));
        // Sin intercalado en el controlador: Pliego repite el juego.
        let sets = plan_copies(3, true, 99, false);
        assert_eq!((sets.device_copies, sets.set_repeats, sets.page_repeats), (1, 3, 1));
        // Sin copias en el controlador y sin intercalar: cada página seguida.
        let pages = plan_copies(3, false, 1, false);
        assert_eq!((pages.device_copies, pages.set_repeats, pages.page_repeats), (1, 1, 3));
        // Sin intercalar el controlador no necesita saber intercalar.
        assert_eq!(plan_copies(2, false, 99, false).device_copies, 2);
        assert_eq!(plan_copies(1, true, 1, false).device_copies, 1);
    }
}
