use serde::{Deserialize, Serialize};

/// Nivel de conformidad PDF/A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Level {
    /// PDF/A-1b (ISO 19005-1, nivel B).
    #[serde(rename = "1b")]
    A1b,
    /// PDF/A-2b (ISO 19005-2, nivel B).
    #[serde(rename = "2b")]
    A2b,
    /// PDF/A-2u: como 2b y además todo el texto se puede convertir a Unicode.
    #[serde(rename = "2u")]
    A2u,
    /// PDF/A-3b (ISO 19005-3, nivel B): permite archivos adjuntos.
    #[serde(rename = "3b")]
    A3b,
}

impl Level {
    /// Parte de la norma (`pdfaid:part`).
    pub fn part(self) -> u8 {
        match self {
            Level::A1b => 1,
            Level::A2b | Level::A2u => 2,
            Level::A3b => 3,
        }
    }
    /// Nivel de conformidad (`pdfaid:conformance`): `B` o `U`.
    pub fn conformance(self) -> &'static str {
        if self == Level::A2u { "U" } else { "B" }
    }
    pub fn name(self) -> &'static str {
        match self {
            Level::A1b => "PDF/A-1b",
            Level::A2b => "PDF/A-2b",
            Level::A2u => "PDF/A-2u",
            Level::A3b => "PDF/A-3b",
        }
    }
    /// ¿Permite transparencia?
    pub fn allows_transparency(self) -> bool {
        self != Level::A1b
    }
    /// ¿Permite archivos incrustados?
    pub fn allows_attachments(self) -> bool {
        self == Level::A3b
    }
    /// ¿Exige correspondencia a Unicode de todo el texto?
    pub fn requires_unicode(self) -> bool {
        self == Level::A2u
    }
}

impl std::str::FromStr for Level {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
            .to_ascii_lowercase()
            .replace(['/', '-', ' '], "")
            .trim_start_matches("pdfa")
        {
            "1b" | "1" => Ok(Level::A1b),
            "2b" | "2" => Ok(Level::A2b),
            "2u" => Ok(Level::A2u),
            "3b" | "3" => Ok(Level::A3b),
            _ => Err(format!("nivel PDF/A desconocido: «{s}» (usa 1b, 2b, 2u o 3b)")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_levels() {
        assert_eq!("1b".parse::<Level>().unwrap(), Level::A1b);
        assert_eq!("PDF/A-2B".parse::<Level>().unwrap(), Level::A2b);
        assert_eq!("pdfa-3b".parse::<Level>().unwrap(), Level::A3b);
        assert_eq!("2u".parse::<Level>().unwrap(), Level::A2u);
        assert!("4b".parse::<Level>().is_err());
        assert_eq!(Level::A2u.part(), 2);
        assert_eq!(Level::A2u.conformance(), "U");
    }
}
