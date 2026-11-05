//! Valores PDF como datos planos ([`Value`]); el objeto con asa es [`crate::Object`], en `native`.

/// Valor PDF como datos planos, para construir estructuras de una vez
/// ([`crate::PdfDocument::object_from_value`]) o leerlas sin asas ([`Object::to_value`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i32),
    Real(f32),
    /// Nombre sin la barra.
    Name(String),
    /// Bytes de una cadena PDF.
    String(Vec<u8>),
    Array(Vec<Value>),
    Dict(Vec<(String, Value)>),
    /// Referencia indirecta `n 0 R`.
    Ref(i32),
}
