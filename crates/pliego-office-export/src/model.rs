//! Modelo intermedio de maquetación: lo que el análisis extrae del texto estructurado y lo que consumen
//! los escritores de DOCX, XLSX y PPTX. Coordenadas en puntos, origen arriba a la izquierda.

pub use pliego_convert::BBox;

/// Campo dinámico dentro de un encabezado o pie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Número de la página actual.
    Page,
    /// Total de páginas.
    NumPages,
}

/// Tramo de texto con un único estilo.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub text: String,
    pub font: String,
    pub size: f32,
    /// `#rrggbb`.
    pub color: String,
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
    pub serif: bool,
    pub field: Option<Field>,
}

impl Run {
    pub(crate) fn from_span(s: &pliego_convert::Span) -> Self {
        Self {
            text: s.text.clone(),
            font: s.font.clone(),
            size: s.size,
            color: s.color.clone(),
            bold: s.bold,
            italic: s.italic,
            monospace: s.monospace,
            serif: s.serif,
            field: None,
        }
    }

    /// Salto de tabulación entre dos fragmentos de una misma fila.
    pub(crate) fn tab(like: &Run) -> Self {
        Self {
            text: "\t".into(),
            field: None,
            ..like.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Decimal,
}

/// Elemento de lista: las viñetas comparten `id` 0; cada lista numerada tiene su propio `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListItem {
    pub kind: ListKind,
    pub id: u32,
    pub start: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Paragraph {
    pub bbox: BBox,
    pub runs: Vec<Run>,
    /// Número de líneas visuales que ocupaba en el PDF.
    pub lines: u32,
    /// Distancia entre líneas base consecutivas (alto de línea si solo hay una).
    pub line_pitch: f32,
    /// x de la primera línea (el marcador, si es una lista).
    pub first_x: f32,
    /// x del texto de las líneas siguientes.
    pub left_x: f32,
    pub align: Align,
    pub list: Option<ListItem>,
    /// Posiciones x (absolutas) de tabuladores; `true` si es de alineación derecha.
    pub tabs: Vec<(f32, bool)>,
}

impl Paragraph {
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Columnas que ocupa.
    pub span: usize,
    pub runs: Vec<Run>,
    pub align: Align,
}

impl Cell {
    pub fn text(&self) -> String {
        self.runs
            .iter()
            .map(|r| r.text.as_str())
            .collect::<String>()
            .trim()
            .to_string()
    }
    pub fn is_empty(&self) -> bool {
        self.text().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub bbox: BBox,
    /// Límites horizontales de cada columna (x0, x1) que teselan la tabla.
    pub cols: Vec<(f32, f32)>,
    /// Filas; cada una suma exactamente `cols.len()` columnas contando `span`.
    pub rows: Vec<Vec<Cell>>,
    /// Alto de cada fila.
    pub row_heights: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageData {
    pub bbox: BBox,
    pub data: Vec<u8>,
    /// `png` o `jpeg`.
    pub ext: &'static str,
    pub px: (u32, u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Paragraph(Paragraph),
    Table(Table),
    Image(ImageData),
}

impl Element {
    pub fn bbox(&self) -> BBox {
        match self {
            Element::Paragraph(p) => p.bbox,
            Element::Table(t) => t.bbox,
            Element::Image(i) => i.bbox,
        }
    }
}

/// Columna de texto dentro de una sección (límites horizontales en la página).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnSpec {
    pub x0: f32,
    pub x1: f32,
}

/// Tramo vertical de una página con 1 o 2 columnas de texto.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub columns: Vec<ColumnSpec>,
    /// Elementos de cada columna en orden de lectura.
    pub content: Vec<Vec<Element>>,
    /// y desde la que empieza esta sección.
    pub top: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageLayout {
    /// Índice de página desde 0.
    pub index: u32,
    pub width: f32,
    pub height: f32,
    pub margins: Margins,
    pub sections: Vec<Section>,
    /// La página es una imagen sin texto (escaneada); su contenido es la imagen a página completa.
    pub scanned: bool,
}

impl PageLayout {
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.sections.iter().flat_map(|s| s.content.iter().flatten())
    }

    pub fn tables(&self) -> impl Iterator<Item = &Table> {
        self.elements().filter_map(|e| match e {
            Element::Table(t) => Some(t),
            _ => None,
        })
    }
}

/// Encabezado o pie repetido en las páginas.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    pub paragraphs: Vec<Paragraph>,
    /// Distancia desde el borde de la página (arriba para encabezado, abajo para pie) hasta el texto.
    pub distance: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layout {
    pub pages: Vec<PageLayout>,
    pub header: Option<Band>,
    pub footer: Option<Band>,
    pub warnings: Vec<String>,
}
