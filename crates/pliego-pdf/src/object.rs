//! Objetos PDF: el modelo de datos del archivo (nulos, números, nombres, cadenas, matrices, diccionarios, flujos y
//! referencias indirectas).
//!
//! # Contrato (válido para cualquier motor)
//!
//! - [`Object`] es un **asa** barata de clonar (`Clone` comparte, `try_clone` también) a un objeto del documento:
//!   una *vista viva*. Modificar el objeto por una asa se ve por las demás asas del mismo objeto indirecto.
//! - **No se debe depender** de que modificar un objeto *después* de pasarlo a [`Object::dict_put`] o
//!   [`Object::array_push`] cambie el que ya está dentro: tras insertar, vuelve a leerlo con `get_dict` /
//!   `get_array` y modifica esa asa. (MuPDF comparte el objeto directo; un motor basado en `lopdf` copia.)
//! - Las asas indirectas (`as_indirect() == Ok(n)`) se resuelven solas al leer (`get_dict`, `is_dict`…); para
//!   conservar la referencia, usa [`Object::as_indirect`] y [`crate::PdfDocument::new_indirect`].
//! - Todas las operaciones devuelven [`pliego_core::Result`]; nunca aparece un tipo de error del motor.
//! - Los números de objeto (`xref`) son los del archivo; el 0 no existe.

use std::fmt;

use pliego_core::{Error, Result};

use crate::backend::{err, matrix_from};
use crate::geom::Matrix;

/// Asa a un objeto PDF. Ver el contrato del módulo.
pub struct Object {
    pub(crate) raw: mupdf::pdf::PdfObject,
}

impl Object {
    pub(crate) fn wrap(raw: mupdf::pdf::PdfObject) -> Self {
        Self { raw }
    }

    // --- Constructores sin documento (valores directos) ---

    pub fn new_null() -> Object {
        Self::wrap(mupdf::pdf::PdfObject::new_null())
    }

    pub fn new_bool(b: bool) -> Object {
        Self::wrap(mupdf::pdf::PdfObject::new_bool(b))
    }

    pub fn new_int(i: i32) -> Result<Object> {
        mupdf::pdf::PdfObject::new_int(i).map(Self::wrap).map_err(err)
    }

    pub fn new_real(f: f32) -> Result<Object> {
        mupdf::pdf::PdfObject::new_real(f).map(Self::wrap).map_err(err)
    }

    /// Cadena PDF (literal) con los bytes de `s` en UTF-8. Para texto con acentos, mejor
    /// [`crate::PdfDocument::new_text_string`] (UTF-16BE con BOM).
    pub fn new_string(s: &str) -> Result<Object> {
        mupdf::pdf::PdfObject::new_string(s).map(Self::wrap).map_err(err)
    }

    pub fn new_name(name: &str) -> Result<Object> {
        mupdf::pdf::PdfObject::new_name(name).map(Self::wrap).map_err(err)
    }

    /// Otra asa al mismo objeto.
    pub fn try_clone(&self) -> Result<Object> {
        self.raw.try_clone().map(Self::wrap).map_err(err)
    }

    // --- Tipos ---

    pub fn is_indirect(&self) -> Result<bool> {
        self.raw.is_indirect().map_err(err)
    }
    pub fn is_null(&self) -> Result<bool> {
        self.raw.is_null().map_err(err)
    }
    pub fn is_bool(&self) -> Result<bool> {
        self.raw.is_bool().map_err(err)
    }
    pub fn is_int(&self) -> Result<bool> {
        self.raw.is_int().map_err(err)
    }
    pub fn is_real(&self) -> Result<bool> {
        self.raw.is_real().map_err(err)
    }
    /// Entero o real.
    pub fn is_number(&self) -> Result<bool> {
        self.raw.is_number().map_err(err)
    }
    pub fn is_string(&self) -> Result<bool> {
        self.raw.is_string().map_err(err)
    }
    pub fn is_name(&self) -> Result<bool> {
        self.raw.is_name().map_err(err)
    }
    pub fn is_array(&self) -> Result<bool> {
        self.raw.is_array().map_err(err)
    }
    pub fn is_dict(&self) -> Result<bool> {
        self.raw.is_dict().map_err(err)
    }
    pub fn is_stream(&self) -> Result<bool> {
        self.raw.is_stream().map_err(err)
    }

    // --- Valores ---

    pub fn as_bool(&self) -> Result<bool> {
        self.raw.as_bool().map_err(err)
    }
    pub fn as_int(&self) -> Result<i32> {
        self.raw.as_int().map_err(err)
    }
    /// Entero o real como `f32`.
    pub fn as_float(&self) -> Result<f32> {
        self.raw.as_float().map_err(err)
    }
    /// Número de objeto de una referencia indirecta.
    pub fn as_indirect(&self) -> Result<i32> {
        self.raw.as_indirect().map_err(err)
    }
    /// Bytes del nombre, sin la barra.
    pub fn as_name(&self) -> Result<Vec<u8>> {
        self.raw.as_name().map_err(err)
    }
    /// Cadena como UTF-8 (falla si los bytes no lo son; usa [`Object::as_bytes`] entonces).
    pub fn as_string(&self) -> Result<String> {
        self.raw.as_string().map_err(err)
    }
    /// Bytes crudos de una cadena PDF.
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        self.raw.as_bytes().map_err(err)
    }

    /// Sigue una referencia indirecta hasta el objeto real (`None` si apunta a la nada).
    pub fn resolve(&self) -> Result<Option<Object>> {
        self.raw.resolve().map(|o| o.map(Self::wrap)).map_err(err)
    }

    // --- Flujos ---

    /// Contenido del flujo, ya descomprimido.
    pub fn read_stream(&self) -> Result<Vec<u8>> {
        self.raw.read_stream().map_err(err)
    }
    /// Contenido del flujo tal como está en el archivo (con sus filtros).
    pub fn read_raw_stream(&self) -> Result<Vec<u8>> {
        self.raw.read_raw_stream().map_err(err)
    }
    /// Sustituye el flujo por `data` **sin comprimir** (quita los filtros del diccionario; el guardado
    /// comprime si se pide).
    pub fn write_stream(&mut self, data: &[u8]) -> Result<()> {
        let buf = mupdf::Buffer::from_bytes(data).map_err(err)?;
        self.raw.write_stream_buffer(&buf).map_err(err)
    }
    /// Sustituye el flujo por `data` tal cual, conservando los filtros del diccionario (`data` ya está codificado).
    pub fn write_raw_stream(&mut self, data: &[u8]) -> Result<()> {
        let buf = mupdf::Buffer::from_bytes(data).map_err(err)?;
        self.raw.write_raw_stream_buffer(&buf).map_err(err)
    }

    // --- Matrices ---

    /// Elemento `index` (desde 0) de una matriz PDF; `None` si no existe.
    pub fn get_array(&self, index: i32) -> Result<Option<Object>> {
        self.raw.get_array(index).map(|o| o.map(Self::wrap)).map_err(err)
    }
    /// Número de elementos de una matriz.
    pub fn len(&self) -> Result<usize> {
        self.raw.len().map_err(err)
    }
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
    /// Elementos de la matriz (asas nuevas, en orden).
    pub fn array_iter(&self) -> Result<Vec<Object>> {
        self.raw
            .array_iter()
            .map_err(err)?
            .map(|r| r.map(Self::wrap).map_err(err))
            .collect()
    }
    pub fn array_push(&mut self, value: Object) -> Result<()> {
        self.raw.array_push(value.raw).map_err(err)
    }
    pub fn array_put(&mut self, index: i32, value: Object) -> Result<()> {
        self.raw.array_put(index, value.raw).map_err(err)
    }
    pub fn array_delete(&mut self, index: i32) -> Result<()> {
        self.raw.array_delete(index).map_err(err)
    }
    /// Copia superficial de una matriz (objeto nuevo, directo).
    pub fn copy_array(&self) -> Result<Object> {
        self.raw.copy_array().map(Self::wrap).map_err(err)
    }

    // --- Diccionarios ---

    /// Valor de `key` (sin barra). `None` si no existe. Las referencias indirectas se siguen.
    pub fn get_dict(&self, key: &str) -> Result<Option<Object>> {
        self.raw.get_dict(key).map(|o| o.map(Self::wrap)).map_err(err)
    }
    /// Como [`Object::get_dict`] pero sube por `/Parent` (atributos heredables de página: `Resources`, `MediaBox`…).
    pub fn get_dict_inheritable(&self, key: &str) -> Result<Option<Object>> {
        self.raw
            .get_dict_inheritable(key)
            .map(|o| o.map(Self::wrap))
            .map_err(err)
    }
    pub fn dict_len(&self) -> Result<usize> {
        self.raw.dict_len().map_err(err)
    }
    /// Clave número `idx` (nombre sin barra).
    pub fn get_dict_key(&self, idx: i32) -> Result<Option<String>> {
        match self.raw.get_dict_key(idx).map_err(err)? {
            Some(k) => Ok(Some(String::from_utf8_lossy(&k.as_name().map_err(err)?).into_owned())),
            None => Ok(None),
        }
    }
    /// Valor número `idx`.
    pub fn get_dict_val(&self, idx: i32) -> Result<Option<Object>> {
        self.raw.get_dict_val(idx).map(|o| o.map(Self::wrap)).map_err(err)
    }
    /// Pares `(clave, valor)` del diccionario, en orden del archivo.
    pub fn dict_iter(&self) -> Result<Vec<(String, Object)>> {
        self.raw
            .dict_iter()
            .map_err(err)?
            .map(|r| {
                let (k, v) = r.map_err(err)?;
                let name = String::from_utf8_lossy(&k.as_name().map_err(err)?).into_owned();
                Ok((name, Self::wrap(v)))
            })
            .collect()
    }
    /// Pone `key = value`. Ver la advertencia del módulo sobre modificar `value` después.
    pub fn dict_put(&mut self, key: &str, value: Object) -> Result<()> {
        self.raw.dict_put(key, value.raw).map_err(err)
    }
    pub fn dict_delete(&mut self, key: &str) -> Result<()> {
        self.raw.dict_delete(key).map_err(err)
    }
    /// Copia superficial de un diccionario (objeto nuevo, directo).
    pub fn copy_dict(&self) -> Result<Object> {
        self.raw.copy_dict().map(Self::wrap).map_err(err)
    }

    // --- Ayudas tipadas (evitan crear objetos intermedios) ---

    pub fn dict_put_name(&mut self, key: &str, name: &str) -> Result<()> {
        self.dict_put(key, Object::new_name(name)?)
    }
    pub fn dict_put_int(&mut self, key: &str, v: i32) -> Result<()> {
        self.dict_put(key, Object::new_int(v)?)
    }
    pub fn dict_put_real(&mut self, key: &str, v: f32) -> Result<()> {
        self.dict_put(key, Object::new_real(v)?)
    }
    pub fn dict_put_bool(&mut self, key: &str, v: bool) -> Result<()> {
        self.dict_put(key, Object::new_bool(v))
    }
    /// Entero de `key`, si existe y es numérico.
    pub fn dict_get_int(&self, key: &str) -> Result<Option<i32>> {
        match self.get_dict(key)? {
            Some(o) if o.is_number()? => Ok(Some(o.as_int().or_else(|_| o.as_float().map(|f| f as i32))?)),
            _ => Ok(None),
        }
    }
    /// Nombre de `key` como cadena, si existe y es un nombre.
    pub fn dict_get_name(&self, key: &str) -> Result<Option<String>> {
        match self.get_dict(key)? {
            Some(o) if o.is_name()? => Ok(Some(String::from_utf8_lossy(&o.as_name()?).into_owned())),
            _ => Ok(None),
        }
    }

    // --- Páginas ---

    /// Matriz que lleva el espacio de usuario de esta página (objeto `/Page`) al espacio de página de salida
    /// (origen arriba a la izquierda, con la rotación aplicada).
    pub fn page_ctm(&self) -> Result<Matrix> {
        self.raw.page_ctm().map(matrix_from).map_err(err)
    }

    /// Sintaxis PDF del objeto (para depurar y para pruebas).
    pub fn to_pdf_string(&self) -> String {
        self.raw.to_string()
    }
}

impl Clone for Object {
    fn clone(&self) -> Self {
        Self { raw: self.raw.clone() }
    }
}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Object({})", self.raw)
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.raw, f)
    }
}

impl From<bool> for Object {
    fn from(b: bool) -> Self {
        Object::new_bool(b)
    }
}

impl TryFrom<i32> for Object {
    type Error = Error;
    fn try_from(v: i32) -> Result<Self> {
        Object::new_int(v)
    }
}

impl TryFrom<f32> for Object {
    type Error = Error;
    fn try_from(v: f32) -> Result<Self> {
        Object::new_real(v)
    }
}

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

impl Object {
    /// Copia el objeto a datos planos, hasta `max_depth` niveles (más abajo queda `Value::Null`). Las referencias
    /// indirectas **no** se siguen: quedan como [`Value::Ref`]. Los flujos se leen como su diccionario.
    pub fn to_value(&self, max_depth: usize) -> Result<Value> {
        if self.is_indirect()? {
            return Ok(Value::Ref(self.as_indirect()?));
        }
        if max_depth == 0 {
            return Ok(Value::Null);
        }
        if self.is_null()? {
            Ok(Value::Null)
        } else if self.is_bool()? {
            Ok(Value::Bool(self.as_bool()?))
        } else if self.is_int()? {
            Ok(Value::Int(self.as_int()?))
        } else if self.is_real()? {
            Ok(Value::Real(self.as_float()?))
        } else if self.is_name()? {
            Ok(Value::Name(String::from_utf8_lossy(&self.as_name()?).into_owned()))
        } else if self.is_string()? {
            Ok(Value::String(self.as_bytes()?))
        } else if self.is_array()? {
            let mut out = Vec::new();
            for i in 0..self.len()? {
                let v = match self.raw_array_elem(i as i32)? {
                    Some(o) => o.to_value(max_depth - 1)?,
                    None => Value::Null,
                };
                out.push(v);
            }
            Ok(Value::Array(out))
        } else if self.is_dict()? {
            let mut out = Vec::new();
            for i in 0..self.dict_len()? {
                let key = self.get_dict_key(i as i32)?.unwrap_or_default();
                let v = match self.raw_dict_val(i as i32)? {
                    Some(o) => o.to_value(max_depth - 1)?,
                    None => Value::Null,
                };
                out.push((key, v));
            }
            Ok(Value::Dict(out))
        } else {
            Ok(Value::Null)
        }
    }

    // `get_array` / `get_dict_val` de MuPDF resuelven las indirectas; para `to_value` hace falta verlas tal cual.
    fn raw_array_elem(&self, i: i32) -> Result<Option<Object>> {
        self.get_array(i)
    }
    fn raw_dict_val(&self, i: i32) -> Result<Option<Object>> {
        self.get_dict_val(i)
    }
}
