//! Preprocesado de imágenes para el reconocimiento: enderezado, binarización adaptativa y eliminación de ruido.
//!
//! Convención de ángulos: positivo = el contenido gira **a favor del reloj** tal y como se ve en pantalla
//! (el eje Y de la imagen apunta hacia abajo).

use crate::GrayImage;

/// Punto de la imagen tras girarla `deg` grados (a favor del reloj) alrededor de `(cx, cy)`.
pub fn rotate_point(x: f32, y: f32, cx: f32, cy: f32, deg: f32) -> (f32, f32) {
    let (s, c) = deg.to_radians().sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    (cx + dx * c - dy * s, cy + dx * s + dy * c)
}

/// Gira la imagen `deg` grados a favor del reloj sobre el mismo lienzo, rellenando con blanco (interpolación bilineal).
pub fn rotate(img: &GrayImage, deg: f32) -> GrayImage {
    if deg.abs() < 1e-3 {
        return img.clone();
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let (cx, cy) = (img.width as f32 / 2.0, img.height as f32 / 2.0);
    let (s, c) = (-deg).to_radians().sin_cos(); // origen = giro inverso del destino
    let mut out = vec![255u8; w * h];
    for y in 0..h {
        let dy = y as f32 + 0.5 - cy;
        for x in 0..w {
            let dx = x as f32 + 0.5 - cx;
            let sx = cx + dx * c - dy * s - 0.5;
            let sy = cy + dx * s + dy * c - 0.5;
            if sx < 0.0 || sy < 0.0 || sx >= (w - 1) as f32 || sy >= (h - 1) as f32 {
                continue;
            }
            let (x0, y0) = (sx as usize, sy as usize);
            let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
            let p = |xx: usize, yy: usize| img.data[yy * w + xx] as f32;
            let v = p(x0, y0) * (1.0 - fx) * (1.0 - fy)
                + p(x0 + 1, y0) * fx * (1.0 - fy)
                + p(x0, y0 + 1) * (1.0 - fx) * fy
                + p(x0 + 1, y0 + 1) * fx * fy;
            out[y * w + x] = v.round() as u8;
        }
    }
    GrayImage {
        width: img.width,
        height: img.height,
        data: out,
    }
}

/// Giro de 90, 180 o 270 grados a favor del reloj (cambia las dimensiones en 90 y 270).
pub fn rotate_quarter(img: &GrayImage, deg: u32) -> GrayImage {
    let (w, h) = (img.width as usize, img.height as usize);
    match deg % 360 {
        90 => {
            let mut out = vec![0u8; w * h];
            for y in 0..h {
                for x in 0..w {
                    out[x * h + (h - 1 - y)] = img.data[y * w + x];
                }
            }
            GrayImage {
                width: img.height,
                height: img.width,
                data: out,
            }
        }
        180 => GrayImage {
            width: img.width,
            height: img.height,
            data: img.data.iter().rev().copied().collect(),
        },
        270 => {
            let mut out = vec![0u8; w * h];
            for y in 0..h {
                for x in 0..w {
                    out[(w - 1 - x) * h + y] = img.data[y * w + x];
                }
            }
            GrayImage {
                width: img.height,
                height: img.width,
                data: out,
            }
        }
        _ => img.clone(),
    }
}

/// Umbral global de Otsu.
pub fn otsu_threshold(img: &GrayImage) -> u8 {
    let mut hist = [0u64; 256];
    for &p in &img.data {
        hist[p as usize] += 1;
    }
    let total = img.data.len() as f64;
    let sum_all: f64 = hist.iter().enumerate().map(|(i, &n)| i as f64 * n as f64).sum();
    let (mut sum_b, mut w_b, mut best, mut thr) = (0.0, 0.0, -1.0, 127u8);
    for (t, &n) in hist.iter().enumerate() {
        w_b += n as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * n as f64;
        let (m_b, m_f) = (sum_b / w_b, (sum_all - sum_b) / w_f);
        let var = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if var > best {
            best = var;
            thr = t as u8;
        }
    }
    thr
}

/// Estima la inclinación del texto en grados (ver convención arriba) buscando el giro que hace más
/// «rayado» el perfil horizontal de tinta. Rango ±`max_deg`; devuelve 0 si no hay texto suficiente.
pub fn estimate_skew(img: &GrayImage, max_deg: f32) -> f32 {
    // Se trabaja sobre una versión reducida (~1000 px de ancho) y solo con los píxeles de tinta.
    let step = (img.width as usize / 1000).max(1);
    let thr = otsu_threshold(img);
    let mut ink: Vec<(f32, f32)> = Vec::new();
    for y in (0..img.height as usize).step_by(step) {
        for x in (0..img.width as usize).step_by(step) {
            if img.data[y * img.width as usize + x] < thr.saturating_sub(10) {
                ink.push((x as f32, y as f32));
            }
        }
    }
    if ink.len() < 200 {
        return 0.0;
    }
    let (cx, cy) = (img.width as f32 / 2.0, img.height as f32 / 2.0);
    let bins = (img.height as usize / step) * 2 + 4;
    let score = |deg: f32| -> f64 {
        // Si el contenido está girado `deg`, lo enderezamos con -deg y medimos el contraste entre filas.
        let (s, c) = (-deg).to_radians().sin_cos();
        let mut hist = vec![0f64; bins];
        for &(x, y) in &ink {
            let ry = (x - cx) * s + (y - cy) * c + cy;
            let idx = (ry / step as f32).round() as isize + 2;
            if idx >= 0 && (idx as usize) < bins {
                hist[idx as usize] += 1.0;
            }
        }
        hist.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum()
    };
    let mut best = (0.0f32, f64::MIN);
    let mut a = -max_deg;
    while a <= max_deg + 1e-3 {
        let sc = score(a);
        if sc > best.1 {
            best = (a, sc);
        }
        a += 0.5;
    }
    let center = best.0;
    let mut a = center - 0.5;
    while a <= center + 0.5 + 1e-3 {
        let sc = score(a);
        if sc > best.1 {
            best = (a, sc);
        }
        a += 0.05;
    }
    best.0
}

/// Binarización adaptativa de Sauvola (ventana `win`, `k` ≈ 0,34). Devuelve 0 (tinta) / 255 (fondo).
pub fn binarize_sauvola(img: &GrayImage, win: usize, k: f32) -> GrayImage {
    let (w, h) = (img.width as usize, img.height as usize);
    let stride = w + 1;
    let mut sum = vec![0u64; stride * (h + 1)];
    let mut sq = vec![0u64; stride * (h + 1)];
    for y in 0..h {
        let (mut rs, mut rq) = (0u64, 0u64);
        for x in 0..w {
            let p = img.data[y * w + x] as u64;
            rs += p;
            rq += p * p;
            sum[(y + 1) * stride + x + 1] = sum[y * stride + x + 1] + rs;
            sq[(y + 1) * stride + x + 1] = sq[y * stride + x + 1] + rq;
        }
    }
    let r = win / 2;
    let mut out = vec![255u8; w * h];
    for y in 0..h {
        let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
            let area = ((y1 - y0) * (x1 - x0)) as f64;
            let s =
                (sum[y1 * stride + x1] + sum[y0 * stride + x0] - sum[y0 * stride + x1] - sum[y1 * stride + x0]) as f64;
            let q = (sq[y1 * stride + x1] + sq[y0 * stride + x0] - sq[y0 * stride + x1] - sq[y1 * stride + x0]) as f64;
            let mean = s / area;
            let std = ((q / area) - mean * mean).max(0.0).sqrt();
            let thr = mean * (1.0 + k as f64 * (std / 128.0 - 1.0));
            if (img.data[y * w + x] as f64) <= thr {
                out[y * w + x] = 0;
            }
        }
    }
    GrayImage {
        width: img.width,
        height: img.height,
        data: out,
    }
}

/// Filtro de mediana 3×3 (quita ruido de «sal y pimienta» sin emborronar los bordes del texto).
pub fn denoise_median3(img: &GrayImage) -> GrayImage {
    let (w, h) = (img.width as usize, img.height as usize);
    let mut out = img.data.clone();
    if w < 3 || h < 3 {
        return img.clone();
    }
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let mut v = [0u8; 9];
            let mut i = 0;
            for yy in y - 1..=y + 1 {
                for xx in x - 1..=x + 1 {
                    v[i] = img.data[yy * w + xx];
                    i += 1;
                }
            }
            v.sort_unstable();
            out[y * w + x] = v[4];
        }
    }
    GrayImage {
        width: img.width,
        height: img.height,
        data: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Página sintética con «renglones» de bloques negros.
    fn lines_page() -> GrayImage {
        let (w, h) = (900u32, 700u32);
        let mut img = GrayImage::filled(w, h, 255);
        for line in 0..12 {
            let y0 = 60 + line * 48;
            for word in 0..9 {
                let x0 = 60 + word * 90;
                for y in y0..y0 + 18 {
                    for x in x0..x0 + 70 {
                        img.data[(y * w + x) as usize] = 0;
                    }
                }
            }
        }
        img
    }

    #[test]
    fn skew_is_detected_and_fixed() {
        let page = lines_page();
        assert!(estimate_skew(&page, 8.0).abs() < 0.3);
        for truth in [-4.0f32, 2.5, 6.0] {
            let skewed = rotate(&page, truth);
            let est = estimate_skew(&skewed, 8.0);
            assert!((est - truth).abs() < 0.4, "verdad {truth}, estimado {est}");
            let fixed = rotate(&skewed, -est);
            assert!(estimate_skew(&fixed, 8.0).abs() < 0.4);
        }
    }

    #[test]
    fn rotate_point_matches_image_rotation() {
        let mut img = GrayImage::filled(200, 200, 255);
        for y in 90..110 {
            for x in 150..170 {
                img.data[y * 200 + x] = 0;
            }
        }
        let rot = rotate(&img, 30.0);
        let (px, py) = rotate_point(160.0, 100.0, 100.0, 100.0, 30.0);
        assert!(rot.get(px as u32, py as u32) < 60);
    }

    #[test]
    fn quarter_turns() {
        let img = GrayImage::new(3, 2, vec![1, 2, 3, 4, 5, 6]).unwrap();
        let r = rotate_quarter(&img, 90);
        assert_eq!((r.width, r.height), (2, 3));
        assert_eq!(r.data, vec![4, 1, 5, 2, 6, 3]);
        assert_eq!(rotate_quarter(&rotate_quarter(&img, 90), 270).data, img.data);
        assert_eq!(rotate_quarter(&img, 180).data, vec![6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn sauvola_handles_uneven_light() {
        // Fondo con degradado + un trazo oscuro: Otsu global fallaría en la zona oscura, Sauvola no.
        let (w, h) = (200u32, 100u32);
        let mut img = GrayImage::filled(w, h, 255);
        for y in 0..h {
            for x in 0..w {
                img.data[(y * w + x) as usize] = (230 - x * 100 / w) as u8;
            }
        }
        for y in 40..60 {
            for x in 150..160 {
                img.data[(y * w + x) as usize] = 60;
            }
        }
        let bin = binarize_sauvola(&img, 25, 0.34);
        assert_eq!(bin.get(155, 50), 0);
        assert_eq!(bin.get(10, 10), 255);
        assert_eq!(bin.get(190, 10), 255);
    }

    #[test]
    fn median_removes_specks() {
        let mut img = GrayImage::filled(20, 20, 255);
        img.data[10 * 20 + 10] = 0;
        assert_eq!(denoise_median3(&img).get(10, 10), 255);
    }
}
