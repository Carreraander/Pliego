use serde::{Deserialize, Serialize};

use crate::js::{Calculation, FieldFormat, Validation};

/// Tipo de campo que se puede crear.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    #[default]
    Text,
    CheckBox,
    RadioButton,
    ComboBox,
    ListBox,
    PushButton,
    Signature,
}

/// Definición de un campo para `add_field` (y resultado de `list_fields` y de la detección automática).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldSpec {
    /// Nombre del campo. No puede contener puntos. Varios radios con el mismo nombre forman un grupo.
    pub name: String,
    pub kind: FieldType,
    /// Página (desde 0).
    pub page: usize,
    /// `[x0, y0, x1, y1]` en espacio de usuario del PDF (origen abajo a la izquierda).
    pub rect: [f32; 4],
    pub required: bool,
    pub read_only: bool,
    /// Valor por defecto: texto, opción elegida, o `true`/`false` en una casilla (se admite el nombre del estado).
    pub value: Option<String>,
    /// Opciones de combo y lista.
    pub options: Vec<String>,
    /// Estado «marcado» de casilla y radio (por defecto `Yes` en casillas; obligatorio en radios).
    pub export_value: Option<String>,
    pub multiline: bool,
    pub password: bool,
    /// Longitud máxima de un campo de texto.
    pub max_len: Option<u32>,
    /// Cuerpo de letra en puntos; 0 = automático.
    pub font_size: f32,
    /// Texto de ayuda (`/TU`).
    pub tooltip: Option<String>,
    /// Rótulo de un botón.
    pub caption: Option<String>,
    /// Formato de presentación y de tecleo (número, fecha, máscara...).
    pub format: FieldFormat,
    /// Validación al confirmar el valor.
    pub validation: Validation,
    /// Cálculo automático a partir de otros campos.
    pub calculation: Option<Calculation>,
}

/// Cambios sobre un campo existente: solo se aplica lo que venga relleno.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldPatch {
    /// Nombre nuevo (sin puntos; sustituye al último componente del nombre).
    pub name: Option<String>,
    /// Nuevo rectángulo; solo en campos de un único widget.
    pub rect: Option<[f32; 4]>,
    pub required: Option<bool>,
    pub read_only: Option<bool>,
    /// Valor por defecto; cadena vacía lo quita.
    pub value: Option<String>,
    pub options: Option<Vec<String>>,
    pub multiline: Option<bool>,
    pub password: Option<bool>,
    /// Longitud máxima; 0 la quita.
    pub max_len: Option<u32>,
    pub font_size: Option<f32>,
    /// Texto de ayuda; cadena vacía lo quita.
    pub tooltip: Option<String>,
}
