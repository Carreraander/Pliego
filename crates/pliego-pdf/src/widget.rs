//! Campos de formulario (AcroForm) de una página.

use std::ops::{BitOr, BitOrAssign};

use pliego_core::Result;

use crate::annot::PdfAnnotation;
use crate::backend::err;
use crate::document::PdfDocument;
use crate::page::PdfPage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetType {
    Unknown,
    Button,
    Checkbox,
    Combobox,
    Listbox,
    RadioButton,
    Signature,
    Text,
}

/// Indicadores `/Ff` del campo (combinables con `|`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FieldFlags(pub i32);

impl FieldFlags {
    pub const READ_ONLY: FieldFlags = FieldFlags(1 << 0);
    pub const REQUIRED: FieldFlags = FieldFlags(1 << 1);
    pub const NO_EXPORT: FieldFlags = FieldFlags(1 << 2);
    pub const MULTILINE: FieldFlags = FieldFlags(1 << 12);
    pub const PASSWORD: FieldFlags = FieldFlags(1 << 13);
    pub const NO_TOGGLE_TO_OFF: FieldFlags = FieldFlags(1 << 14);
    pub const RADIO: FieldFlags = FieldFlags(1 << 15);
    pub const PUSHBUTTON: FieldFlags = FieldFlags(1 << 16);
    pub const COMBO: FieldFlags = FieldFlags(1 << 17);
    pub const EDIT: FieldFlags = FieldFlags(1 << 18);
    pub const SORT: FieldFlags = FieldFlags(1 << 19);
    pub const FILE_SELECT: FieldFlags = FieldFlags(1 << 20);
    pub const MULTI_SELECT: FieldFlags = FieldFlags(1 << 21);
    pub const DO_NOT_SPELL_CHECK: FieldFlags = FieldFlags(1 << 22);
    pub const DO_NOT_SCROLL: FieldFlags = FieldFlags(1 << 23);
    pub const COMB: FieldFlags = FieldFlags(1 << 24);
    pub const RADIOS_IN_UNISON: FieldFlags = FieldFlags(1 << 25);
    pub const COMMIT_ON_SEL_CHANGE: FieldFlags = FieldFlags(1 << 26);
    pub const RICH_TEXT: FieldFlags = FieldFlags(1 << 25);

    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn bits(self) -> i32 {
        self.0
    }
    pub const fn contains(self, o: FieldFlags) -> bool {
        self.0 & o.0 == o.0
    }
}

impl BitOr for FieldFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for FieldFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Campo de formulario. Se obtiene de [`PdfPage::widgets`] / [`PdfPage::load_widget`].
pub struct PdfWidget {
    annot: PdfAnnotation,
}

impl PdfWidget {
    pub(crate) fn wrap(raw: mupdf::pdf::PdfWidget) -> Self {
        Self {
            annot: PdfAnnotation::of_widget(raw),
        }
    }

    fn w(&self) -> &mupdf::pdf::PdfWidget {
        self.annot.widget().expect("PdfWidget siempre contiene un campo")
    }

    fn w_mut(&mut self) -> &mut mupdf::pdf::PdfWidget {
        self.annot.widget_mut().expect("PdfWidget siempre contiene un campo")
    }

    /// Anotación `/Widget` del campo, en solo lectura (rectángulo, diccionario…). Para modificarla,
    /// [`PdfWidget::into_annotation`].
    pub fn annotation(&self) -> &PdfAnnotation {
        &self.annot
    }

    pub fn into_annotation(self) -> Result<PdfAnnotation> {
        self.annot.into_inner().map(PdfAnnotation::wrap)
    }

    pub fn xref(&self) -> Result<i32> {
        self.w().xref().map_err(err)
    }

    pub fn r#type(&self) -> Result<WidgetType> {
        use mupdf::pdf::WidgetType as W;
        self.w()
            .r#type()
            .map(|t| match t {
                W::Button => WidgetType::Button,
                W::Checkbox => WidgetType::Checkbox,
                W::Combobox => WidgetType::Combobox,
                W::Listbox => WidgetType::Listbox,
                W::RadioButton => WidgetType::RadioButton,
                W::Signature => WidgetType::Signature,
                W::Text => WidgetType::Text,
                _ => WidgetType::Unknown,
            })
            .map_err(err)
    }

    /// Regenera la apariencia. `true` si cambió.
    pub fn update(&mut self) -> Result<bool> {
        self.w_mut().update().map_err(err)
    }

    pub fn is_signed(&self) -> Result<bool> {
        self.w().is_signed().map_err(err)
    }

    pub fn is_readonly(&self) -> Result<bool> {
        self.w().is_readonly().map_err(err)
    }

    /// Nombre calificado del campo (`persona.nombre`).
    pub fn name(&self) -> Result<Option<String>> {
        self.w().name().map_err(err)
    }

    /// Tipo de campo según `/FT` (`"Tx"`, `"Btn"`, `"Ch"`, `"Sig"`).
    pub fn field_type_name(&self) -> Result<Option<String>> {
        self.w().field_type_name().map_err(err)
    }

    pub fn field_flags(&self) -> Result<FieldFlags> {
        self.w().field_flags().map(|f| FieldFlags(f.bits())).map_err(err)
    }

    pub fn value(&self) -> Result<Option<String>> {
        self.w().value().map_err(err)
    }

    /// Texto de ayuda (`/TU`).
    pub fn label(&self) -> Result<Option<String>> {
        self.w().label().map_err(err)
    }

    /// Fija el valor. `ignore_trigger_events`: no ejecutar los scripts de formato y validación. Devuelve `false`
    /// si el motor lo rechaza (p. ej. por una validación).
    pub fn set_value(&mut self, doc: &mut PdfDocument, value: &str, ignore_trigger_events: bool) -> Result<bool> {
        self.w_mut()
            .set_value(doc.pdf_mut(), value, ignore_trigger_events)
            .map_err(err)
    }

    pub fn reset(&mut self, doc: &mut PdfDocument) -> Result<()> {
        self.w_mut().reset(doc.pdf_mut()).map_err(err)
    }
}

impl PdfPage {
    /// Campos de la página.
    pub fn widgets(&self) -> std::vec::IntoIter<PdfWidget> {
        self.pdf()
            .widgets()
            .map(PdfWidget::wrap)
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub fn load_widget(&self, xref: i32) -> Result<Option<PdfWidget>> {
        self.pdf()
            .load_widget(xref)
            .map(|w| w.map(PdfWidget::wrap))
            .map_err(err)
    }

    pub fn delete_widget(&mut self, widget: PdfWidget) -> Result<()> {
        let annot = widget.into_annotation()?;
        self.delete_annotation(annot)
    }

    /// Añade un campo de firma vacío (sin rectángulo: colócalo con su anotación).
    pub fn add_signature_widget(&mut self, name: &str) -> Result<PdfWidget> {
        self.pdf_mut()
            .add_signature_widget(name)
            .map(PdfWidget::wrap)
            .map_err(err)
    }
}
