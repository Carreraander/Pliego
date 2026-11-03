use serde::{Deserialize, Serialize};

/// Gravedad de un hallazgo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Incumple PDF/UA o impide usar el documento con un lector de pantalla.
    Error,
    /// Mala práctica o no se puede comprobar de forma automática.
    Warning,
}

/// Código estable de cada comprobación (para filtrar, traducir y enlazar correcciones).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    NotTagged,
    NoStructureTree,
    NoLanguage,
    InvalidLanguage,
    NoTitle,
    TitleNotDisplayed,
    FigureWithoutAlt,
    HeadingSkipped,
    FirstHeadingNotH1,
    NoHeadings,
    TableWithoutHeader,
    FieldWithoutLabel,
    LinkWithoutDescription,
    NoTabOrder,
    NoBookmarks,
    LowContrast,
    CopyRestricted,
    TruncatedStructure,
}

/// Un hallazgo con su corrección sugerida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct A11yIssue {
    pub code: IssueCode,
    pub severity: Severity,
    /// Página (desde 0) donde se encontró, si aplica.
    pub page: Option<usize>,
    /// Objeto afectado (número de objeto PDF), por ejemplo la figura sin texto alternativo.
    pub xref: Option<i32>,
    pub message: String,
    pub suggestion: String,
}

/// Resultado de [`crate::check`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct A11yReport {
    pub pages: usize,
    pub tagged: bool,
    pub language: Option<String>,
    pub title: Option<String>,
    pub issues: Vec<A11yIssue>,
}

impl A11yReport {
    pub fn errors(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == Severity::Error).count()
    }
    pub fn warnings(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == Severity::Warning).count()
    }
    /// Sin errores (puede tener avisos).
    pub fn passes(&self) -> bool {
        self.errors() == 0
    }
    pub fn has(&self, code: IssueCode) -> bool {
        self.issues.iter().any(|i| i.code == code)
    }
}
