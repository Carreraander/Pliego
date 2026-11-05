//! Tipos de opciones y resultados de las operaciones de documento: guardado, cifrado, inserción de páginas,
//! etiquetas, archivos incrustados y capas.

use std::ops::{BitOr, BitOrAssign, Range, RangeInclusive};

use pliego_core::{Error, Result};

/// Cifrado al guardar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Encryption {
    /// Conservar el cifrado del original (por defecto; un documento abierto sin cifrar se guarda sin cifrar).
    #[default]
    Keep,
    /// Quitar el cifrado.
    None,
    Rc4_40,
    Rc4_128,
    Aes128,
    Aes256,
}

/// Permisos de un documento cifrado: los bits de `/P` de PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Permission(pub u32);

impl Permission {
    pub const PRINT: Permission = Permission(1 << 2);
    pub const MODIFY: Permission = Permission(1 << 3);
    pub const COPY: Permission = Permission(1 << 4);
    pub const ANNOTATE: Permission = Permission(1 << 5);
    pub const FORM: Permission = Permission(1 << 8);
    pub const ACCESSIBILITY: Permission = Permission(1 << 9);
    pub const ASSEMBLE: Permission = Permission(1 << 10);
    pub const PRINT_HQ: Permission = Permission(1 << 11);

    pub const fn empty() -> Self {
        Permission(0)
    }

    pub const fn all() -> Self {
        Permission(
            Self::PRINT.0
                | Self::MODIFY.0
                | Self::COPY.0
                | Self::ANNOTATE.0
                | Self::FORM.0
                | Self::ACCESSIBILITY.0
                | Self::ASSEMBLE.0
                | Self::PRINT_HQ.0,
        )
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Permission) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: Permission) {
        self.0 |= other.0;
    }
}

impl BitOr for Permission {
    type Output = Permission;
    fn bitor(self, rhs: Permission) -> Permission {
        Permission(self.0 | rhs.0)
    }
}

impl BitOrAssign for Permission {
    fn bitor_assign(&mut self, rhs: Permission) {
        self.0 |= rhs.0;
    }
}

/// Opciones de guardado. Los métodos `set_*` devuelven `&mut Self` para encadenar.
///
/// `Default` equivale a escribir el documento completo, sin comprimir ni limpiar y conservando el cifrado
/// (idéntico al valor por defecto de MuPDF).
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    pub(crate) incremental: bool,
    pub(crate) pretty: bool,
    pub(crate) ascii: bool,
    pub(crate) compress: bool,
    pub(crate) compress_images: bool,
    pub(crate) compress_fonts: bool,
    pub(crate) decompress: bool,
    pub(crate) garbage_level: i32,
    pub(crate) linear: bool,
    pub(crate) clean: bool,
    pub(crate) sanitize: bool,
    pub(crate) appearance: bool,
    pub(crate) object_streams: bool,
    pub(crate) encryption: Encryption,
    pub(crate) permissions: Option<Permission>,
    pub(crate) owner_password: String,
    pub(crate) user_password: String,
}

macro_rules! flag_setter {
    ($(#[$m:meta])* $name:ident, $getter:ident, $field:ident) => {
        $(#[$m])*
        pub fn $name(&mut self, value: bool) -> &mut Self {
            self.$field = value;
            self
        }
        pub fn $getter(&self) -> bool {
            self.$field
        }
    };
}

impl WriteOptions {
    pub fn new() -> Self {
        Self::default()
    }

    flag_setter!(
        /// Añade solo los cambios al final del archivo original (conserva firmas). Exige que se haya abierto
        /// desde un archivo y [`crate::PdfDocument::can_be_saved_incrementally`].
        set_incremental, incremental, incremental);
    flag_setter!(set_pretty, pretty, pretty);
    flag_setter!(set_ascii, ascii, ascii);
    flag_setter!(
        /// Comprime los flujos sin comprimir.
        set_compress, compress, compress);
    flag_setter!(set_compress_images, compress_images, compress_images);
    flag_setter!(set_compress_fonts, compress_fonts, compress_fonts);
    flag_setter!(
        /// Descomprime los flujos (para inspeccionar contenido).
        set_decompress, decompress, decompress);
    flag_setter!(set_linear, linear, linear);
    flag_setter!(set_clean, clean, clean);
    flag_setter!(set_sanitize, sanitize, sanitize);
    flag_setter!(set_appearance, appearance, appearance);
    flag_setter!(
        /// Escribe flujos de objetos y de referencias (PDF 1.5, archivos más pequeños; no válido para PDF/A-1).
        /// Solo lo aplica el guardado con el modelo propio (`native`); MuPDF lo ignora.
        set_object_streams, object_streams, object_streams);

    /// Elimina objetos no usados (nivel 1). `false` desactiva la limpieza.
    pub fn set_garbage(&mut self, value: bool) -> &mut Self {
        self.garbage_level = i32::from(value);
        self
    }

    pub fn garbage(&self) -> bool {
        self.garbage_level > 0
    }

    /// 0 = nada, 1 = objetos no usados, 2 = renumerar, 3 = deduplicar objetos, 4 = deduplicar flujos.
    pub fn set_garbage_level(&mut self, level: i32) -> &mut Self {
        self.garbage_level = level.clamp(0, 4);
        self
    }

    pub fn garbage_level(&self) -> i32 {
        self.garbage_level
    }

    pub fn set_encryption(&mut self, value: Encryption) -> &mut Self {
        self.encryption = value;
        self
    }

    pub fn encryption(&self) -> Encryption {
        self.encryption
    }

    pub fn set_permissions(&mut self, value: Permission) -> &mut Self {
        self.permissions = Some(value);
        self
    }

    /// Permisos que se escribirán (todos si no se fijaron).
    pub fn permissions(&self) -> Permission {
        self.permissions.unwrap_or_else(Permission::all)
    }

    pub fn set_owner_password(&mut self, pwd: &str) -> &mut Self {
        self.owner_password = pwd.to_owned();
        self
    }

    pub fn set_user_password(&mut self, pwd: &str) -> &mut Self {
        self.user_password = pwd.to_owned();
        self
    }
}

/// Rango de páginas `[start, end)` desde 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRange {
    pub start: usize,
    pub end: usize,
}

impl PageRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

impl From<Range<usize>> for PageRange {
    fn from(r: Range<usize>) -> Self {
        Self::new(r.start, r.end)
    }
}

impl From<RangeInclusive<usize>> for PageRange {
    fn from(r: RangeInclusive<usize>) -> Self {
        let (s, e) = r.into_inner();
        Self::new(s, e.saturating_add(1))
    }
}

/// Qué páginas de un documento.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PageSelection {
    #[default]
    All,
    Range(PageRange),
    /// Páginas sueltas, en el orden dado (puede repetir).
    Pages(Vec<usize>),
}

impl PageSelection {
    pub(crate) fn validated_indices(&self, page_count: usize) -> Result<Vec<usize>> {
        let v: Vec<usize> = match self {
            Self::All => (0..page_count).collect(),
            Self::Range(r) => {
                if r.is_empty() {
                    return Err(Error::InvalidInput("el rango de páginas está vacío".into()));
                }
                (r.start..r.end).collect()
            }
            Self::Pages(p) => p.clone(),
        };
        if let Some(bad) = v.iter().find(|&&i| i >= page_count) {
            return Err(Error::InvalidInput(format!(
                "la página {} no existe (hay {page_count})",
                bad + 1
            )));
        }
        Ok(v)
    }
}

impl From<PageRange> for PageSelection {
    fn from(r: PageRange) -> Self {
        Self::Range(r)
    }
}

impl From<Range<usize>> for PageSelection {
    fn from(r: Range<usize>) -> Self {
        Self::Range(r.into())
    }
}

impl From<Vec<usize>> for PageSelection {
    fn from(p: Vec<usize>) -> Self {
        Self::Pages(p)
    }
}

impl From<&[usize]> for PageSelection {
    fn from(p: &[usize]) -> Self {
        Self::Pages(p.to_vec())
    }
}

/// Dónde insertar páginas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InsertPosition {
    #[default]
    Append,
    /// Antes de la página `n` (desde 0).
    Before(usize),
    /// Después de la página `n` (desde 0).
    After(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertPdfOptions {
    pub source_pages: PageSelection,
    pub target: InsertPosition,
    /// Giro adicional (múltiplo de 90) para las páginas insertadas.
    pub rotate: Option<i32>,
}

impl Default for InsertPdfOptions {
    fn default() -> Self {
        Self {
            source_pages: PageSelection::All,
            target: InsertPosition::Append,
            rotate: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsertPdfResult {
    pub inserted_pages: PageRange,
    pub page_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageLabelStyle {
    None,
    Decimal,
    UpperRoman,
    LowerRoman,
    UpperAlpha,
    LowerAlpha,
}

/// Regla de etiquetas de página que empieza en la página `index` (desde 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageLabelRule {
    pub index: usize,
    pub style: PageLabelStyle,
    pub prefix: String,
    pub start: i32,
}

impl PageLabelRule {
    pub fn new(index: usize, style: PageLabelStyle) -> Self {
        Self {
            index,
            style,
            prefix: String::new(),
            start: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedFileOptions<'a> {
    pub filename: &'a str,
    pub mime_type: Option<&'a str>,
    /// Segundos Unix.
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub add_checksum: bool,
}

impl<'a> EmbeddedFileOptions<'a> {
    pub fn new(filename: &'a str) -> Self {
        Self {
            filename,
            mime_type: None,
            created: None,
            modified: None,
            add_checksum: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedFileInfo {
    /// Nombre en el árbol de nombres `/EmbeddedFiles`.
    pub name: String,
    pub xref: i32,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub size: usize,
    pub created: Option<i64>,
    pub modified: Option<i64>,
}

/// Capa (grupo de contenido opcional).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalContentGroup {
    /// Número de objeto del grupo.
    pub xref: i32,
    pub name: Option<String>,
    pub enabled: bool,
}

/// Metadatos estándar legibles con [`crate::Document::metadata`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataName {
    Format,
    Encryption,
    Author,
    Title,
    Producer,
    Creator,
    CreationDate,
    ModDate,
    Subject,
    Keywords,
}

/// Esquema de marcadores del documento.
#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub title: String,
    pub uri: Option<String>,
    pub dest: Option<crate::links::LinkDestination>,
    pub down: Vec<Outline>,
}
