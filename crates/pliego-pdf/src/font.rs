//! Tipos comunes de fuentes; la fuente en sí es [`crate::Font`], en `native`.

/// Recorrido de un contorno de glifo o de un trazado.
pub trait PathWalker {
    fn move_to(&mut self, x: f32, y: f32);
    fn line_to(&mut self, x: f32, y: f32);
    fn curve_to(&mut self, cx1: f32, cy1: f32, cx2: f32, cy2: f32, ex: f32, ey: f32);
    fn close(&mut self);
}

/// Codificación de una fuente simple incrustada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleFontEncoding {
    Latin,
    Greek,
    Cyrillic,
}
