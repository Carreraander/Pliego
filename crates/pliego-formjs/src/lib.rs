//! Motor de JavaScript de formularios PDF.
//!
//! Ejecuta las acciones de campo de un AcroForm (tecleo, formato, validación y cálculo; acciones de ratón y de
//! documento) con el modelo de objetos de Acrobat: `event`, `this.getField(..)`, `app`, `util`, `color` y las funciones
//! `AF*`. El intérprete es [Boa](https://boajs.dev) (Rust puro, MIT/Unlicense); este crate **no depende de ningún
//! crate de PDF**: trabaja sobre un [`Form`] en memoria y devuelve [`Change`]s que quien lo use aplica al documento.
//!
//! No ejecuta nada con efectos fuera del formulario: enviar el formulario, abrir URL, escribir archivos o imprimir
//! quedan anotados como [`Message::Blocked`]. No hay temporizadores ni XFA.

mod engine;
pub mod format;
mod model;

pub use engine::{Config, Engine};
pub use format::Language;
pub use model::{Change, Color, Field, FieldKind, Form, Message, Outcome, Prop, ScriptError, flags};
