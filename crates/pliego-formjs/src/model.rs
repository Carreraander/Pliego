//! Modelo del formulario que el motor ve: una copia en memoria de los campos y de sus scripts. El motor no sabe
//! nada de PDF; quien lo use construye un [`Form`] desde el documento y aplica de vuelta los [`Change`]s.

use std::collections::BTreeMap;

/// Tipo de campo (lo que Acrobat llama `field.type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    Text,
    Checkbox,
    Radio,
    Button,
    Combo,
    List,
    Signature,
}

impl FieldKind {
    /// Nombre que devuelve `field.type`.
    pub fn js_name(self) -> &'static str {
        match self {
            FieldKind::Text => "text",
            FieldKind::Checkbox => "checkbox",
            FieldKind::Radio => "radiobutton",
            FieldKind::Button => "button",
            FieldKind::Combo => "combobox",
            FieldKind::List => "listbox",
            FieldKind::Signature => "signature",
        }
    }
}

/// Color de PDF (`/MK /BG`, `/BC`, `/DA`): transparente, gris, RGB o CMYK con componentes de 0 a 1.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Color {
    #[default]
    Transparent,
    Gray(f64),
    Rgb(f64, f64, f64),
    Cmyk(f64, f64, f64, f64),
}

/// Valor de una propiedad libre (las que el motor no modela como campo propio).
#[derive(Debug, Clone, PartialEq)]
pub enum Prop {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Prop>),
}

/// Bits de `/Ff` que `field.*` expone como propiedades.
pub mod flags {
    pub const READ_ONLY: u32 = 1;
    pub const REQUIRED: u32 = 1 << 1;
    pub const NO_EXPORT: u32 = 1 << 2;
    pub const MULTILINE: u32 = 1 << 12;
    pub const PASSWORD: u32 = 1 << 13;
    pub const NO_TOGGLE_TO_OFF: u32 = 1 << 14;
    pub const RADIO: u32 = 1 << 15;
    pub const PUSHBUTTON: u32 = 1 << 16;
    pub const COMBO: u32 = 1 << 17;
    pub const EDIT: u32 = 1 << 18;
    pub const SORT: u32 = 1 << 19;
    pub const FILE_SELECT: u32 = 1 << 20;
    pub const MULTI_SELECT: u32 = 1 << 21;
    pub const DO_NOT_SPELL_CHECK: u32 = 1 << 22;
    pub const DO_NOT_SCROLL: u32 = 1 << 23;
    pub const COMB: u32 = 1 << 24;
    pub const RICH_TEXT: u32 = 1 << 25;
    pub const COMMIT_ON_SEL_CHANGE: u32 = 1 << 26;
}

/// Un campo terminal del formulario (el que tiene valor).
#[derive(Debug, Clone)]
pub struct Field {
    /// Nombre completo con puntos (`Datos.Nombre`).
    pub name: String,
    pub kind: FieldKind,
    /// Valor actual como texto. Casillas y radios: el valor de exportación o `Off`.
    pub value: String,
    /// Selección múltiple de una lista (vacía si solo hay un valor).
    pub multi: Vec<String>,
    pub default_value: String,
    /// Opciones de combinados y listas: (valor de exportación, etiqueta).
    pub options: Vec<(String, String)>,
    /// Valores de exportación de los widgets de una casilla o un radio, en orden (sin `Off`).
    pub export_values: Vec<String>,
    /// Bits de `/Ff`.
    pub flags: u32,
    /// `0` visible, `1` oculto, `2` visible pero no se imprime, `3` oculto en pantalla pero se imprime.
    pub display: u8,
    pub max_len: u32,
    /// Página (desde 0) y rectángulo `[x0, y0, x1, y1]` en el espacio de la página.
    pub page: u32,
    pub rect: [f64; 4],
    pub text_size: f64,
    pub text_font: String,
    pub text_color: Color,
    pub fill_color: Color,
    pub stroke_color: Color,
    /// `/Q`: 0 izquierda, 1 centro, 2 derecha.
    pub alignment: u8,
    pub tooltip: String,
    pub border_style: String,
    pub line_width: f64,
    /// Acciones por clave de `/AA` (`K`, `F`, `V`, `C`, `E`, `X`, `D`, `U`, `Fo`, `Bl`...) y `A` para el botón.
    pub scripts: BTreeMap<String, String>,
    /// Propiedades que el motor no modela aparte (rótulo de botón, `submitName`...).
    pub extra: BTreeMap<String, Prop>,
    /// Texto que debe verse tras aplicar el formato (`/AA /F`); `None`: se ve el valor tal cual.
    pub display_text: Option<String>,
}

impl Field {
    pub fn new(name: impl Into<String>, kind: FieldKind) -> Self {
        Field {
            name: name.into(),
            kind,
            value: String::new(),
            multi: Vec::new(),
            default_value: String::new(),
            options: Vec::new(),
            export_values: Vec::new(),
            flags: 0,
            display: 0,
            max_len: 0,
            page: 0,
            rect: [0.0; 4],
            text_size: 0.0,
            text_font: "Helv".into(),
            text_color: Color::Gray(0.0),
            fill_color: Color::Transparent,
            stroke_color: Color::Gray(0.0),
            alignment: 0,
            tooltip: String::new(),
            border_style: "solid".into(),
            line_width: 1.0,
            scripts: BTreeMap::new(),
            extra: BTreeMap::new(),
            display_text: None,
        }
    }

    /// Texto que debe verse: el formateado si lo hay, o el valor.
    pub fn shown(&self) -> &str {
        self.display_text.as_deref().unwrap_or(&self.value)
    }

    pub fn has_flag(&self, f: u32) -> bool {
        self.flags & f != 0
    }

    /// ¿Se comporta como número? (tiene un formato numérico: `field.value` devuelve entonces un `Number`).
    pub fn is_numeric(&self) -> bool {
        self.scripts
            .get("F")
            .is_some_and(|s| s.contains("AFNumber_Format") || s.contains("AFPercent_Format"))
    }
}

/// Todo el formulario, más lo que los scripts preguntan del documento.
#[derive(Debug, Clone, Default)]
pub struct Form {
    pub fields: Vec<Field>,
    /// Orden de cálculo (`/AcroForm /CO`): nombres de campo.
    pub calc_order: Vec<String>,
    /// Scripts del documento (`/Names /JavaScript`): (nombre, código), en orden.
    pub doc_scripts: Vec<(String, String)>,
    pub page_count: u32,
    pub file_name: String,
    pub title: String,
}

impl Form {
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut Field> {
        self.fields.iter_mut().find(|f| f.name == name)
    }
}

/// Qué cambió en el formulario mientras corrían los scripts.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// El valor de un campo (`Field::value`/`multi` ya lo tienen).
    Value { field: String },
    /// Texto que debe verse en el campo, ya con el formato aplicado. `None`: el campo se ve con su valor.
    Display { field: String, text: Option<String> },
    /// Una propiedad (`textColor`, `hidden`, `readonly`...) cambió; la lectura está en el campo.
    Prop { field: String, prop: String },
    /// Se añadió o cambió una acción del campo (`setAction`).
    Script { field: String, trigger: String },
}

/// Avisos y efectos que el motor **no** ejecuta (no hay interfaz ni red) y deja a quien lo use.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// `app.alert`. `kind`: 0 Aceptar, 1 Aceptar/Cancelar, 2 Sí/No, 3 Sí/No/Cancelar; `icon`: 0 error, 1 aviso,
    /// 2 pregunta, 3 estado.
    Alert {
        text: String,
        title: String,
        icon: u8,
        kind: u8,
    },
    /// `console.println` y errores de script.
    Console(String),
    /// Una acción bloqueada por seguridad: enviar el formulario, abrir una URL, escribir un archivo...
    Blocked { action: String, detail: String },
}

/// Error en un script. Un script con error no impide el resto: se anota y sigue.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptError {
    pub field: Option<String>,
    pub trigger: String,
    pub message: String,
}

/// Resultado de una operación del motor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    /// `false` si una validación o el teclado rechazaron el valor (`event.rc = false`).
    pub accepted: bool,
    pub changes: Vec<Change>,
    pub messages: Vec<Message>,
    pub errors: Vec<ScriptError>,
    /// Resultado de texto de la operación: en `keystroke`, el cambio ya filtrado por los scripts.
    pub text: Option<String>,
}

impl Outcome {
    pub fn accepted() -> Self {
        Outcome {
            accepted: true,
            ..Default::default()
        }
    }

    /// Nombres de los campos cuyo valor cambió, sin repetir y en orden.
    pub fn changed_values(&self) -> Vec<&str> {
        let mut v: Vec<&str> = Vec::new();
        for c in &self.changes {
            if let Change::Value { field } = c
                && !v.contains(&field.as_str())
            {
                v.push(field);
            }
        }
        v
    }

    /// Texto de visualización fijado para un campo (la última vez).
    pub fn display_of(&self, name: &str) -> Option<&Option<String>> {
        self.changes.iter().rev().find_map(|c| match c {
            Change::Display { field, text } if field == name => Some(text),
            _ => None,
        })
    }
}
