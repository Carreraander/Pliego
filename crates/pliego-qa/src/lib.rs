//! Herramientas de QA de Pliego: arnés de comparación de motores y oráculo de redacción.
//!
//! - [`compare`]: abre cada PDF con dos motores, renderiza a 72 y 150 ppp, y mide la diferencia de píxeles y de texto.
//! - [`oracle`]: comprueba que un PDF redactado no conserva los datos (texto, flujos, imágenes bajo el área).
//! - [`validate`]: validadores externos opcionales (`qpdf --check`, veraPDF).
//! - [`engines`]: registro de motores por nombre; aquí se enchufará el motor nuevo.

pub mod bigpdf;
pub mod compare;
pub mod engines;
pub mod oracle;
pub mod validate;
