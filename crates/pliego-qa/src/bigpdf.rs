//! Generador de PDF grandes para el banco de memoria (hueco P1 #11 de `docs/handoff/huecos-1.0.md`).
//!
//! Escribe en streaming (nunca guarda el PDF entero en memoria), así que sirve para 1 GB o más:
//!
//! - [`write_scan`]: «escaneado». Una imagen JPEG por página (ruido, incompresible) más una capa de texto invisible
//!   (`Tr 3`, como deja el OCR) para poder buscar. El tamaño total sale de `pages × image_bytes`.
//! - [`write_text`]: muchas páginas de texto (5 000 por defecto en el banco), como `tools/gen_bench_pdf.py`.
//!
//! Los dos escriben un PDF 1.4 con tabla de referencias clásica, con objetos en orden, y son deterministas.

use std::io::{self, BufWriter, Seek, Write};
use std::path::Path;

use jpeg_encoder::{ColorType, Encoder};

/// Generador congruencial lineal: ruido determinista y barato, sin dependencias.
struct Noise(u64);

impl Noise {
    fn byte(&mut self) -> u8 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 56) as u8
    }
}

/// JPEG en gris de ruido de unos `target_bytes` bytes (±15 %). Ruido = casi sin compresión, como un escaneo con grano.
pub fn noise_jpeg(target_bytes: usize) -> io::Result<Vec<u8>> {
    let encode = |side: u16| -> io::Result<Vec<u8>> {
        let mut n = Noise(0x5eed_u64 ^ u64::from(side));
        let pixels: Vec<u8> = (0..usize::from(side) * usize::from(side)).map(|_| n.byte()).collect();
        let mut out = Vec::new();
        Encoder::new(&mut out, 85)
            .encode(&pixels, side, side, ColorType::Luma)
            .map_err(io::Error::other)?;
        Ok(out)
    };
    // Muestra pequeña para calcular los bytes por píxel y luego un solo ajuste al lado necesario.
    let sample = encode(256)?;
    let per_pixel = sample.len() as f64 / (256.0 * 256.0);
    let side = ((target_bytes as f64 / per_pixel).sqrt().round() as u32).clamp(64, 16_000) as u16;
    encode(side)
}

/// Dimensiones (ancho, alto) en píxeles de un JPEG baseline: lo mínimo para declararlo en el PDF.
fn jpeg_size(jpeg: &[u8]) -> Option<(u16, u16)> {
    let mut i = 2;
    while i + 9 < jpeg.len() {
        if jpeg[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = jpeg[i + 1];
        let len = usize::from(u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]));
        if (0xC0..=0xC3).contains(&marker) {
            let h = u16::from_be_bytes([jpeg[i + 5], jpeg[i + 6]]);
            let w = u16::from_be_bytes([jpeg[i + 7], jpeg[i + 8]]);
            return Some((w, h));
        }
        i += 2 + len;
    }
    None
}

/// Escritor de objetos que apunta los desplazamientos para la tabla de referencias.
struct Out<W: Write + Seek> {
    w: BufWriter<W>,
    pos: u64,
    offsets: Vec<u64>,
}

impl<W: Write + Seek> Out<W> {
    fn new(w: W) -> io::Result<Self> {
        let mut o = Self {
            w: BufWriter::with_capacity(1 << 20, w),
            pos: 0,
            offsets: Vec::new(),
        };
        // Cabecera con bytes altos: así los programas no lo toman por texto.
        o.raw(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n")?;
        Ok(o)
    }

    fn raw(&mut self, b: &[u8]) -> io::Result<()> {
        self.w.write_all(b)?;
        self.pos += b.len() as u64;
        Ok(())
    }

    /// Empieza el objeto `n` (se numeran en orden desde 1).
    fn begin(&mut self, n: usize) -> io::Result<()> {
        assert_eq!(n, self.offsets.len() + 1, "los objetos se escriben en orden");
        self.offsets.push(self.pos);
        self.raw(format!("{n} 0 obj\n").as_bytes())
    }

    fn end(&mut self) -> io::Result<()> {
        self.raw(b"\nendobj\n")
    }

    fn object(&mut self, n: usize, body: &str) -> io::Result<()> {
        self.begin(n)?;
        self.raw(body.as_bytes())?;
        self.end()
    }

    fn stream(&mut self, n: usize, dict: &str, data: &[u8]) -> io::Result<()> {
        self.begin(n)?;
        self.raw(format!("<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes())?;
        self.raw(data)?;
        self.raw(b"\nendstream")?;
        self.end()
    }

    fn finish(mut self, root: usize) -> io::Result<()> {
        let xref = self.pos;
        let count = self.offsets.len() + 1;
        let mut table = format!("xref\n0 {count}\n0000000000 65535 f \n");
        for off in &self.offsets {
            table.push_str(&format!("{off:010} 00000 n \n"));
        }
        table.push_str(&format!(
            "trailer\n<< /Size {count} /Root {root} 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ));
        self.raw(table.as_bytes())?;
        self.w.flush()
    }
}

/// Resultado de una generación.
#[derive(Debug, Clone, Copy)]
pub struct Generated {
    pub pages: usize,
    pub bytes: u64,
}

/// PDF «escaneado»: `pages` páginas A4, cada una con su imagen de ~`image_bytes` bytes (JPEG de ruido) y texto invisible
/// `Escaneo <n> contrato` para buscar. Con 300 páginas y 3,5 MB por imagen salen ~1 GB.
pub fn write_scan(path: &Path, pages: usize, image_bytes: usize) -> io::Result<Generated> {
    let jpeg = noise_jpeg(image_bytes)?;
    let (w, h) = jpeg_size(&jpeg).ok_or_else(|| io::Error::other("JPEG generado no válido"))?;
    let file = std::fs::File::create(path)?;
    let mut out = Out::new(file)?;
    // 1 catálogo, 2 árbol, 3 fuente; cada página usa tres objetos seguidos: página, imagen y contenido.
    let page_obj = |p: usize| 4 + 3 * p;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    let kids: Vec<String> = (0..pages).map(|p| format!("{} 0 R", page_obj(p))).collect();
    out.object(
        2,
        &format!("<< /Type /Pages /Count {pages} /Kids [{}] >>", kids.join(" ")),
    )?;
    out.object(3, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")?;
    for p in 0..pages {
        let (page, image, content) = (page_obj(p), page_obj(p) + 1, page_obj(p) + 2);
        out.object(
            page,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents {content} 0 R \
                 /Resources << /XObject << /Im0 {image} 0 R >> /Font << /F1 3 0 R >> >> >>"
            ),
        )?;
        out.stream(
            image,
            &format!("/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /DCTDecode"),
            &jpeg,
        )?;
        let text = format!("Escaneo {} contrato", p + 1);
        let content_stream = format!("q 595 0 0 842 0 0 cm /Im0 Do Q\nBT 3 Tr /F1 12 Tf 56 780 Td ({text}) Tj ET\n");
        out.stream(content, "", content_stream.as_bytes())?;
    }
    out.finish(1)?;
    Ok(Generated {
        pages,
        bytes: std::fs::metadata(path)?.len(),
    })
}

/// PDF de texto de `pages` páginas con 52 líneas cada una (como `tools/gen_bench_pdf.py`) y un marcador por cada diez
/// páginas, para que el árbol de marcadores también pese.
pub fn write_text(path: &Path, pages: usize) -> io::Result<Generated> {
    let file = std::fs::File::create(path)?;
    let mut out = Out::new(file)?;
    let page_obj = |p: usize| 4 + 2 * p;
    let outline_first = 4 + 2 * pages;
    let marks = pages.div_ceil(10);
    out.object(
        1,
        &format!("<< /Type /Catalog /Pages 2 0 R /Outlines {outline_first} 0 R /PageMode /UseOutlines >>"),
    )?;
    let kids: Vec<String> = (0..pages).map(|p| format!("{} 0 R", page_obj(p))).collect();
    out.object(
        2,
        &format!("<< /Type /Pages /Count {pages} /Kids [{}] >>", kids.join(" ")),
    )?;
    out.object(3, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")?;
    for p in 0..pages {
        let (page, content) = (page_obj(p), page_obj(p) + 1);
        out.object(
            page,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents {content} 0 R \
                 /Resources << /Font << /F1 3 0 R >> >> >>"
            ),
        )?;
        let mut s = String::from("BT /F1 11 Tf 56 780 Td 14 TL\n");
        for i in 0..52 {
            s.push_str(&format!(
                "(Pagina {}, linea {}: pliego es un visor y editor de PDF libre para Windows.) Tj T*\n",
                p + 1,
                i + 1
            ));
        }
        s.push_str("ET\n");
        out.stream(content, "", s.as_bytes())?;
    }
    // Marcadores: raíz (outline_first) y uno por cada diez páginas.
    out.object(
        outline_first,
        &format!(
            "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {marks} >>",
            outline_first + 1,
            outline_first + marks
        ),
    )?;
    for m in 0..marks {
        let id = outline_first + 1 + m;
        let mut d = format!(
            "<< /Title (Capitulo {}) /Parent {outline_first} 0 R /Dest [{} 0 R /Fit]",
            m + 1,
            page_obj(m * 10)
        );
        if m > 0 {
            d.push_str(&format!(" /Prev {} 0 R", id - 1));
        }
        if m + 1 < marks {
            d.push_str(&format!(" /Next {} 0 R", id + 1));
        }
        d.push_str(" >>");
        out.object(id, &d)?;
    }
    out.finish(1)?;
    Ok(Generated {
        pages,
        bytes: std::fs::metadata(path)?.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_jpeg_is_near_target_and_valid() {
        let j = noise_jpeg(200_000).unwrap();
        assert!((150_000..260_000).contains(&j.len()), "{}", j.len());
        let (w, h) = jpeg_size(&j).unwrap();
        assert_eq!(w, h);
    }

    #[test]
    fn scan_and_text_open_with_lopdf_layer() {
        let dir = tempfile::tempdir().unwrap();
        let scan = dir.path().join("scan.pdf");
        let g = write_scan(&scan, 4, 50_000).unwrap();
        assert_eq!(g.pages, 4);
        assert!(g.bytes > 4 * 40_000);
        let doc = pliego_pdf::PdfDocument::open(&scan).unwrap();
        assert_eq!(doc.page_count().unwrap(), 4);

        let text = dir.path().join("text.pdf");
        write_text(&text, 25).unwrap();
        let doc = pliego_pdf::PdfDocument::open(&text).unwrap();
        assert_eq!(doc.page_count().unwrap(), 25);
        assert_eq!(doc.outlines().unwrap().len(), 3);
    }
}
