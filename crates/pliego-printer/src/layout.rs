//! Maquetación de una página PDF en la hoja. Son cuentas puras (sin Windows, sin PDFium): las usan el envío a la
//! impresora y la vista previa del diálogo, de modo que lo que se ve es lo que sale.

use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::options::{Booklet, NupOrder, Orientation, PrintOptions, ScaleMode, Subset};

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

/// Una página dibujada en una hoja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    /// Página del documento (desde 0).
    pub page: u32,
    pub placement: Placement,
}

/// Lo que se imprime en una cara de una hoja: una o varias páginas ya colocadas. Sin páginas es una hoja en blanco.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetPlan {
    pub slots: Vec<Slot>,
    /// Dibujar un marco alrededor de cada página.
    pub borders: bool,
}

/// Separación entre páginas de una misma hoja, en puntos.
const GUTTER_PT: f64 = 8.0;
/// Una página girada para caber mejor tiene que ganar al menos esto para que compense leerla de lado.
const ROTATION_PENALTY: f64 = 0.98;

/// Escala a la que cabe `page` (ancho, alto) en una celda, girándola si `auto_rotate` y compensa.
fn cell_fit(page: (f64, f64), cell: (f64, f64), auto_rotate: bool) -> f64 {
    let fit = |(w, h): (f64, f64)| (cell.0 / w).min(cell.1 / h);
    let upright = fit(page);
    if auto_rotate {
        upright.max(fit((page.1, page.0)) * ROTATION_PENALTY)
    } else {
        upright
    }
}

/// Mejor cuadrícula `(columnas, filas)` de `n` celdas para la página y la zona dadas, con su escala. Entre dos igual de
/// buenas gana la que sigue la forma de la zona (más columnas si es apaisada).
fn best_grid(n: u32, page: (f64, f64), area: (f64, f64), auto_rotate: bool) -> ((u32, u32), f64) {
    let mut pairs: Vec<(u32, u32)> = (1..=n).filter(|c| n.is_multiple_of(*c)).map(|c| (c, n / c)).collect();
    if area.0 >= area.1 {
        pairs.reverse();
    }
    let mut best = (pairs[0], f64::MIN);
    for (c, r) in pairs {
        let score = cell_fit(page, (area.0 / f64::from(c), area.1 / f64::from(r)), auto_rotate);
        if score > best.1 * (1.0 + 1e-9) {
            best = ((c, r), score);
        }
    }
    best
}

/// Orientación de la hoja para el trabajo: la elegida, o la que mejor aprovecha la hoja. Con una página por hoja es la
/// de la primera página que se imprime; con varias por hoja, la que da páginas más grandes (el folleto va siempre
/// apaisado). Devuelve `true` si es apaisada.
pub fn choose_landscape(options: &PrintOptions, first_page: (f32, f32)) -> bool {
    match options.orientation {
        Orientation::Portrait => false,
        Orientation::Landscape => true,
        Orientation::Auto if options.booklet != Booklet::Off => true,
        Orientation::Auto if options.pages_per_sheet > 1 => {
            // Solo importa la forma de la hoja: un papel con proporción de A4.
            let side = std::f64::consts::SQRT_2;
            let page = (f64::from(first_page.0).max(1.0), f64::from(first_page.1).max(1.0));
            let score = |area| best_grid(options.pages_per_sheet, page, area, options.auto_rotate).1;
            score((side, 1.0)) > score((1.0, side)) * 1.0001
        }
        Orientation::Auto => first_page.0 > first_page.1,
    }
}

/// Páginas de cada cara que se imprime, en el orden de impresión; `None` es un hueco en blanco (la última hoja de un
/// folleto o de una cuadrícula incompleta).
pub fn sheet_groups(options: &PrintOptions, page_count: u32) -> Result<Vec<Vec<Option<u32>>>> {
    options.validate()?;
    let empty = || Error::InvalidInput("no hay ninguna página que imprimir con esa selección".into());
    let mut sheets: Vec<Vec<Option<u32>>> = if options.booklet != Booklet::Off {
        let pages = options.base_selection(page_count)?;
        // Un cuadernillo son hojas de cuatro páginas: se completa con páginas en blanco al final.
        let len = pages.len().div_ceil(4) * 4;
        let at = |i: usize| pages.get(i).copied();
        let mut sides = Vec::new();
        for k in 0..len / 4 {
            // Anverso: contraportada a la izquierda y portada a la derecha; reverso: las dos páginas de dentro.
            let mut front = vec![at(len - 1 - 2 * k), at(2 * k)];
            let mut back = vec![at(2 * k + 1), at(len - 2 - 2 * k)];
            if options.booklet_rtl {
                front.reverse();
                back.reverse();
            }
            if options.booklet != Booklet::BackOnly {
                sides.push(front);
            }
            if options.booklet != Booklet::FrontOnly {
                sides.push(back);
            }
        }
        // Una cara sin ninguna página (el reverso de un folleto de una sola página) no gasta papel.
        sides.retain(|s| s.iter().any(Option::is_some));
        sides
    } else if options.pages_per_sheet <= 1 {
        options
            .selection(page_count)?
            .into_iter()
            .map(|p| vec![Some(p)])
            .collect()
    } else {
        let pages = options.base_selection(page_count)?;
        let mut sheets: Vec<Vec<Option<u32>>> = pages
            .chunks(options.pages_per_sheet as usize)
            .map(|c| c.iter().map(|&p| Some(p)).collect())
            .collect();
        // Pares e impares cuentan hojas (para imprimir a doble cara a mano): la hoja 1 es impar.
        let mut index = 0;
        sheets.retain(|_| {
            let keep = match options.subset {
                Subset::All => true,
                Subset::Odd => index % 2 == 0,
                Subset::Even => index % 2 == 1,
            };
            index += 1;
            keep
        });
        sheets
    };
    if options.reverse && (options.booklet != Booklet::Off || options.pages_per_sheet > 1) {
        sheets.reverse();
    }
    if sheets.is_empty() {
        return Err(empty());
    }
    Ok(sheets)
}

/// Posición `(columna, fila)` de la celda `i` de una cuadrícula de `cols × rows` según el orden pedido.
fn cell_position(i: u32, cols: u32, rows: u32, order: NupOrder) -> (u32, u32) {
    let (col, row) = match order {
        NupOrder::Horizontal | NupOrder::HorizontalReversed => (i % cols, i / cols),
        NupOrder::Vertical | NupOrder::VerticalReversed => (i / rows, i % rows),
    };
    match order {
        NupOrder::HorizontalReversed | NupOrder::VerticalReversed => (cols - 1 - col, row),
        _ => (col, row),
    }
}

/// Coloca todas las caras del trabajo en la hoja `sheet`. Con una página por hoja es exactamente [`place`]; con varias
/// (o en folleto) cada página se ajusta a su celda de la cuadrícula.
pub fn plan_sheets(sizes: &[(f32, f32)], sheet: &SheetMetrics, options: &PrintOptions) -> Result<Vec<SheetPlan>> {
    let groups = sheet_groups(options, sizes.len() as u32)?;
    if !options.is_multi_up() {
        return Ok(groups
            .iter()
            .map(|g| {
                let page = g[0].unwrap_or(0);
                let (w, h) = sizes[page as usize];
                SheetPlan {
                    slots: vec![Slot {
                        page,
                        placement: place(w, h, sheet, options),
                    }],
                    borders: false,
                }
            })
            .collect());
    }
    let first = groups.iter().flatten().flatten().next().copied().unwrap_or(0);
    let (fw, fh) = sizes[first as usize];
    let (cols, rows) = if options.booklet != Booklet::Off {
        (2, 1)
    } else {
        best_grid(
            options.pages_per_sheet,
            (f64::from(fw).max(1.0), f64::from(fh).max(1.0)),
            (sheet.area_w, sheet.area_h),
            options.auto_rotate,
        )
        .0
    };
    let gutter_x = (GUTTER_PT * sheet.dpi_x / 72.0).min(sheet.area_w * 0.02);
    let gutter_y = (GUTTER_PT * sheet.dpi_y / 72.0).min(sheet.area_h * 0.02);
    let cell_w = (sheet.area_w - gutter_x * f64::from(cols - 1)) / f64::from(cols);
    let cell_h = (sheet.area_h - gutter_y * f64::from(rows - 1)) / f64::from(rows);
    let cell = SheetMetrics {
        phys_w: cell_w,
        phys_h: cell_h,
        off_x: 0.0,
        off_y: 0.0,
        area_w: cell_w,
        area_h: cell_h,
        ..*sheet
    };
    // Dentro de una celda siempre se ajusta: tamaño real o porcentaje no tienen sentido con varias páginas.
    let fit = PrintOptions {
        scale: ScaleMode::Fit,
        ..options.clone()
    };
    let order = if options.booklet != Booklet::Off {
        NupOrder::Horizontal
    } else {
        options.nup_order
    };
    Ok(groups
        .iter()
        .map(|group| {
            let slots = group
                .iter()
                .enumerate()
                .filter_map(|(i, page)| {
                    let page = (*page)?;
                    let (col, row) = cell_position(i as u32, cols, rows, order);
                    let (w, h) = sizes[page as usize];
                    let mut placement = place(w, h, &cell, &fit);
                    placement.x += ((cell_w + gutter_x) * f64::from(col)).round() as i32;
                    placement.y += ((cell_h + gutter_y) * f64::from(row)).round() as i32;
                    Some(Slot { page, placement })
                })
                .collect();
            SheetPlan {
                slots,
                borders: options.borders,
            }
        })
        .collect())
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
    /// A doble cara, con un número impar de caras por juego: se añade una cara en blanco entre juegos para que el
    /// siguiente juego empiece en una hoja nueva y no en el reverso de la última.
    pub pad_odd_sets: bool,
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
            pad_odd_sets: false,
        };
    }
    if max_copies >= requested && (!collate || can_collate) {
        return CopiesPlan {
            device_copies: requested,
            device_collate: collate,
            set_repeats: 1,
            page_repeats: 1,
            pad_odd_sets: false,
        };
    }
    CopiesPlan {
        device_copies: 1,
        device_collate: false,
        set_repeats: if collate { requested } else { 1 },
        page_repeats: if collate { 1 } else { requested },
        pad_odd_sets: false,
    }
}

/// Como [`plan_copies`] para un trabajo que puede ir a doble cara. Repetir cada cara seguida (sin intercalar) pondría
/// la misma cara en anverso y reverso, así que a doble cara Pliego intercala siempre las copias que hace él.
pub fn plan_copies_for(requested: u32, collate: bool, max_copies: u32, can_collate: bool, duplex: bool) -> CopiesPlan {
    let mut plan = plan_copies(requested, collate, max_copies, can_collate);
    if duplex && plan.page_repeats > 1 {
        plan = plan_copies(requested, true, max_copies, can_collate);
    }
    plan.pad_odd_sets = duplex && plan.set_repeats > 1;
    plan
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

    fn groups(o: &PrintOptions, n: u32) -> Vec<Vec<Option<u32>>> {
        sheet_groups(o, n).unwrap()
    }

    fn booklet(mode: Booklet) -> PrintOptions {
        PrintOptions {
            booklet: mode,
            ..PrintOptions::default()
        }
    }

    #[test]
    fn booklet_of_eight_pages_pairs_covers_and_inner_pages() {
        let both = groups(&booklet(Booklet::Both), 8);
        // Páginas (desde 1): 8|1, 2|7, 6|3, 4|5.
        let one_based: Vec<Vec<u32>> = both
            .iter()
            .map(|s| s.iter().map(|p| p.unwrap() + 1).collect())
            .collect();
        assert_eq!(one_based, vec![vec![8, 1], vec![2, 7], vec![6, 3], vec![4, 5]]);
        let fronts = groups(&booklet(Booklet::FrontOnly), 8);
        assert_eq!(fronts, vec![vec![Some(7), Some(0)], vec![Some(5), Some(2)]]);
        let backs = groups(&booklet(Booklet::BackOnly), 8);
        assert_eq!(backs, vec![vec![Some(1), Some(6)], vec![Some(3), Some(4)]]);
    }

    #[test]
    fn booklet_pads_with_blanks_and_drops_empty_sides() {
        // 5 páginas → 8 con tres en blanco al final.
        let g = groups(&booklet(Booklet::Both), 5);
        assert_eq!(g[0], vec![None, Some(0)]);
        assert_eq!(g[1], vec![Some(1), None]);
        assert_eq!(g.len(), 4);
        // Una sola página: el reverso estaría vacío y no sale.
        assert_eq!(groups(&booklet(Booklet::Both), 1), vec![vec![None, Some(0)]]);
    }

    #[test]
    fn booklet_right_to_left_swaps_each_side() {
        let o = PrintOptions {
            booklet_rtl: true,
            ..booklet(Booklet::Both)
        };
        assert_eq!(groups(&o, 4)[0], vec![Some(0), Some(3)]);
        assert_eq!(groups(&o, 4)[1], vec![Some(2), Some(1)]);
    }

    #[test]
    fn booklet_reverse_flips_the_order_of_sides() {
        let o = PrintOptions {
            reverse: true,
            ..booklet(Booklet::FrontOnly)
        };
        assert_eq!(groups(&o, 8)[0], vec![Some(5), Some(2)]);
    }

    #[test]
    fn n_up_groups_pages_and_subset_counts_sheets() {
        let o = PrintOptions {
            pages_per_sheet: 4,
            ..PrintOptions::default()
        };
        let g = groups(&o, 10);
        assert_eq!(g.len(), 3);
        assert_eq!(g[2], vec![Some(8), Some(9)]);
        let odd = PrintOptions {
            subset: Subset::Odd,
            ..o.clone()
        };
        assert_eq!(groups(&odd, 10).len(), 2);
        assert_eq!(groups(&odd, 10)[1][0], Some(8));
        let rev = PrintOptions { reverse: true, ..o };
        assert_eq!(groups(&rev, 10)[0][0], Some(8));
        assert_eq!(groups(&rev, 10)[2][0], Some(0));
    }

    #[test]
    fn grid_follows_the_shape_of_the_sheet_and_the_page() {
        // Dos páginas verticales caben mejor lado a lado en una hoja apaisada.
        let (grid, _) = best_grid(2, (595.0, 842.0), (842.0, 595.0), true);
        assert_eq!(grid, (2, 1));
        // Cuatro en una hoja vertical: 2 × 2.
        assert_eq!(best_grid(4, (595.0, 842.0), (595.0, 842.0), true).0, (2, 2));
        // Seis páginas verticales en una hoja vertical: 2 columnas × 3 filas.
        assert_eq!(best_grid(6, (595.0, 842.0), (595.0, 842.0), true).0, (2, 3));
        // Nueve y dieciséis son cuadradas.
        assert_eq!(best_grid(9, (595.0, 842.0), (595.0, 842.0), true).0, (3, 3));
        assert_eq!(best_grid(16, (595.0, 842.0), (595.0, 842.0), true).0, (4, 4));
    }

    #[test]
    fn auto_orientation_for_n_up_and_booklet() {
        let two = PrintOptions {
            pages_per_sheet: 2,
            ..PrintOptions::default()
        };
        // Dos páginas A4 verticales: hoja apaisada, cada una derecha en su mitad.
        assert!(choose_landscape(&two, (595.0, 842.0)));
        let four = PrintOptions {
            pages_per_sheet: 4,
            ..PrintOptions::default()
        };
        assert!(!choose_landscape(&four, (595.0, 842.0)));
        // Cuatro páginas apaisadas caben mejor en una hoja apaisada.
        assert!(choose_landscape(&four, (842.0, 595.0)));
        assert!(choose_landscape(&booklet(Booklet::Both), (595.0, 842.0)));
        let forced = PrintOptions {
            orientation: Orientation::Portrait,
            ..two
        };
        assert!(!choose_landscape(&forced, (595.0, 842.0)));
    }

    #[test]
    fn single_page_per_sheet_plan_is_the_plain_placement() {
        let sheet = SheetMetrics::plain(595.0, 842.0);
        let sizes = [(595.0, 842.0), (842.0, 595.0)];
        let plans = plan_sheets(&sizes, &sheet, &PrintOptions::default()).unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(
            plans[1].slots[0].placement,
            place(842.0, 595.0, &sheet, &PrintOptions::default())
        );
        assert!(!plans[0].borders);
    }

    #[test]
    fn two_up_on_landscape_sheet_places_pages_side_by_side_without_overlap() {
        let sheet = SheetMetrics::plain(842.0, 595.0);
        let sizes = [(595.0, 842.0); 3];
        let o = PrintOptions {
            pages_per_sheet: 2,
            borders: true,
            scale: ScaleMode::Actual, // se ignora con varias por hoja
            ..PrintOptions::default()
        };
        let plans = plan_sheets(&sizes, &sheet, &o).unwrap();
        assert_eq!(plans.len(), 2);
        assert!(plans[0].borders);
        let [a, b] = [plans[0].slots[0], plans[0].slots[1]];
        assert_eq!((a.page, b.page), (0, 1));
        assert!(a.placement.x + a.placement.w <= b.placement.x, "{a:?} {b:?}");
        assert!(b.placement.x + b.placement.w <= 842);
        assert!(a.placement.h <= 595 && a.placement.rotate == 0);
        // La hoja incompleta solo lleva su página.
        assert_eq!(plans[1].slots.len(), 1);
    }

    #[test]
    fn cell_order_variants() {
        // Cuadrícula 2 × 2.
        let pos = |order| (0..4).map(|i| cell_position(i, 2, 2, order)).collect::<Vec<_>>();
        assert_eq!(pos(NupOrder::Horizontal), [(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert_eq!(pos(NupOrder::HorizontalReversed), [(1, 0), (0, 0), (1, 1), (0, 1)]);
        assert_eq!(pos(NupOrder::Vertical), [(0, 0), (0, 1), (1, 0), (1, 1)]);
        assert_eq!(pos(NupOrder::VerticalReversed), [(1, 0), (1, 1), (0, 0), (0, 1)]);
    }

    #[test]
    fn booklet_sheet_has_two_cells_and_a_back_with_the_inner_pages() {
        let sheet = SheetMetrics::plain(842.0, 595.0);
        let sizes = [(595.0, 842.0); 4];
        let plans = plan_sheets(&sizes, &sheet, &booklet(Booklet::Both)).unwrap();
        let pages = |i: usize| plans[i].slots.iter().map(|s| s.page).collect::<Vec<_>>();
        assert_eq!((pages(0), pages(1)), (vec![3, 0], vec![1, 2]));
        assert!(plans[0].slots[0].placement.x < plans[0].slots[1].placement.x);
    }

    #[test]
    fn duplex_copies_are_always_collated_when_pliego_makes_them() {
        // Sin duplex: cada página seguida, como siempre.
        assert_eq!(plan_copies_for(3, false, 1, false, false).page_repeats, 3);
        // A doble cara: juegos completos, con una cara en blanco si el juego es impar.
        let d = plan_copies_for(3, false, 1, false, true);
        assert_eq!((d.page_repeats, d.set_repeats, d.pad_odd_sets), (1, 3, true));
        // Si el controlador hace las copias, no hay nada que rellenar.
        let hw = plan_copies_for(3, false, 99, false, true);
        assert_eq!((hw.device_copies, hw.pad_odd_sets), (3, false));
    }
}
