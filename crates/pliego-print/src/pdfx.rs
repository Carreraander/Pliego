//! Conversión a PDF/X con Ghostscript como herramienta externa opcional.
//!
//! Pliego no incluye Ghostscript (AGPL, pero es un programa aparte) ni perfiles ICC: la persona usuaria elige el
//! perfil de la condición de impresión (FOGRA39, SWOP, etc.) y, si Ghostscript no está instalado, recibe un
//! error que lo explica.

use std::path::{Path, PathBuf};
use std::process::Command;

use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfxLevel {
    /// PDF/X-1a:2001: solo CMYK y colores directos, sin transparencias.
    X1a,
    /// PDF/X-4. De mejor esfuerzo: Ghostscript no escribe los metadatos XMP que exige el estándar; conviene
    /// validar el resultado con una herramienta externa antes de enviarlo a imprenta.
    X4,
}

impl PdfxLevel {
    fn version(self) -> &'static str {
        match self {
            Self::X1a => "PDF/X-1a:2001",
            Self::X4 => "PDF/X-4",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfxOptions {
    pub level: PdfxLevel,
    /// Perfil ICC CMYK de la condición de impresión (obligatorio).
    pub icc_profile: PathBuf,
    /// Identificador registrado, p. ej. `FOGRA39`.
    pub condition_identifier: String,
    /// Descripción legible de la condición de impresión.
    pub condition: String,
    pub title: Option<String>,
    /// Ruta de Ghostscript; si es `None` se busca en el `PATH` y en `Program Files`.
    pub ghostscript: Option<PathBuf>,
}

/// Escapa una cadena PostScript.
fn ps_string(s: &str) -> String {
    let mut out = String::from("(");
    for c in s.chars() {
        match c {
            '\\' | '(' | ')' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push(')');
    out
}

/// Programa PostScript que define la intención de salida y la versión de PDF/X.
pub(crate) fn pdfx_definition(opts: &PdfxOptions, icc: &str) -> String {
    let conformance = match opts.level {
        PdfxLevel::X1a => " /GTS_PDFXConformance (PDF/X-1a:2001)",
        PdfxLevel::X4 => "",
    };
    format!(
        "%!\n\
         [ /GTS_PDFXVersion ({version}){conformance} /Title {title} /Trapped /False /DOCINFO pdfmark\n\
         [/_objdef {{icc_PDFX}} /type /stream /OBJ pdfmark\n\
         [{{icc_PDFX}} << /N 4 >> /PUT pdfmark\n\
         [{{icc_PDFX}} {icc} (r) file /PUT pdfmark\n\
         [/_objdef {{OutputIntent_PDFX}} /type /dict /OBJ pdfmark\n\
         [{{OutputIntent_PDFX}} << /Type /OutputIntent /S /GTS_PDFX /OutputCondition {condition} /Info {condition} \
         /OutputConditionIdentifier {id} /RegistryName (http://www.color.org) /DestOutputProfile {{icc_PDFX}} >> /PUT pdfmark\n\
         [{{Catalog}} << /OutputIntents [ {{OutputIntent_PDFX}} ] >> /PUT pdfmark\n",
        version = opts.level.version(),
        title = ps_string(opts.title.as_deref().unwrap_or("Documento")),
        icc = ps_string(icc),
        condition = ps_string(&opts.condition),
        id = ps_string(&opts.condition_identifier),
    )
}

/// Busca Ghostscript en el `PATH` y, en Windows, en `Program Files\gs`.
pub fn find_ghostscript() -> Option<PathBuf> {
    const NAMES: [&str; 4] = ["gswin64c.exe", "gswin32c.exe", "gs", "gs.exe"];
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for n in NAMES {
                let p = dir.join(n);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    for root in ["ProgramFiles", "ProgramFiles(x86)"] {
        let Some(base) = std::env::var_os(root) else { continue };
        let Ok(entries) = std::fs::read_dir(Path::new(&base).join("gs")) else {
            continue;
        };
        let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        versions.sort();
        for v in versions.iter().rev() {
            for n in ["gswin64c.exe", "gswin32c.exe"] {
                let p = v.join("bin").join(n);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Comprueba que el archivo es un perfil ICC de salida CMYK.
fn check_profile(path: &Path) -> Result<()> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    if bytes.len() < 128 || &bytes[36..40] != b"acsp" {
        return Err(Error::InvalidInput(format!(
            "{} no es un perfil ICC válido",
            path.display()
        )));
    }
    if &bytes[16..20] != b"CMYK" {
        return Err(Error::InvalidInput(
            "el perfil ICC debe ser CMYK (el de la condición de impresión, p. ej. FOGRA39)".into(),
        ));
    }
    Ok(())
}

/// Ruta absoluta con barras normales, sin el prefijo `\\?\` de Windows, apta para PostScript.
fn ps_path(path: &Path) -> Result<String> {
    let abs = std::fs::canonicalize(path).map_err(|e| Error::io_at(path, e))?;
    let s = abs
        .to_str()
        .ok_or_else(|| Error::InvalidInput("la ruta del perfil debe ser UTF-8".into()))?;
    Ok(s.trim_start_matches(r"\\?\").replace('\\', "/"))
}

/// Convierte `input` a PDF/X en `output` (que no puede ser el propio `input`).
///
/// Todo el color se convierte a CMYK, se incrusta la intención de salida y se declara la versión de PDF/X.
/// Requiere Ghostscript; usa [`find_ghostscript`] para saber si está disponible.
pub fn to_pdfx(input: &Path, output: &Path, opts: &PdfxOptions, ctx: &JobContext<'_>) -> Result<Report> {
    if !input.exists() {
        return Err(Error::io_at(input, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    if std::fs::canonicalize(input).ok() == std::fs::canonicalize(output).ok() && output.exists() {
        return Err(Error::InvalidInput(
            "el archivo de salida es el mismo que el de entrada; elige otro nombre para no perder el original".into(),
        ));
    }
    check_profile(&opts.icc_profile)?;
    let gs = opts.ghostscript.clone().or_else(find_ghostscript).ok_or_else(|| {
        Error::Unsupported(
            "para convertir a PDF/X hace falta Ghostscript (gratuito, https://ghostscript.com); instálalo o indica \
             su ruta"
                .into(),
        )
    })?;
    ctx.step(0, 2, "Convirtiendo a PDF/X con Ghostscript")?;

    let icc = ps_path(&opts.icc_profile)?;
    let work = tempfile::tempdir()?;
    let def = work.path().join("pdfx_def.ps");
    std::fs::write(&def, pdfx_definition(opts, &icc)).map_err(|e| Error::io_at(&def, e))?;
    let dir = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io_at(dir, e))?;

    let result = Command::new(&gs)
        .args([
            "-dBATCH",
            "-dNOPAUSE",
            "-dQUIET",
            "-dSAFER",
            "-sDEVICE=pdfwrite",
            "-dPDFX",
        ])
        .args(["-sColorConversionStrategy=CMYK", "-dProcessColorModel=/DeviceCMYK"])
        .arg(format!("--permit-file-read={icc}"))
        .arg(format!("-sOutputFile={}", tmp.path().display()))
        .arg(&def)
        .arg(input)
        .output()
        .map_err(|e| Error::Other(format!("no se pudo ejecutar Ghostscript ({}): {e}", gs.display())))?;
    if !result.status.success() {
        let detail = String::from_utf8_lossy(&result.stderr);
        let stdout = String::from_utf8_lossy(&result.stdout);
        let first = detail
            .lines()
            .chain(stdout.lines())
            .find(|l| !l.trim().is_empty())
            .unwrap_or("sin detalle");
        return Err(Error::Other(format!(
            "Ghostscript no pudo convertir el documento: {first}"
        )));
    }
    ctx.step(1, 2, "Guardando")?;
    tmp.persist(output).map_err(|e| Error::io_at(output, e.error))?;

    let mut report = Report::with_output(
        output,
        format!(
            "Convertido a {} con la condición «{}».",
            opts.level.version(),
            opts.condition_identifier
        ),
    );
    if opts.level == PdfxLevel::X4 {
        report.warnings.push(
            "PDF/X-4 de mejor esfuerzo: Ghostscript no escribe los metadatos XMP del estándar; valida el archivo con \
             una herramienta de preflight antes de enviarlo."
                .into(),
        );
    }
    ctx.step(2, 2, "Terminado")?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(level: PdfxLevel) -> PdfxOptions {
        PdfxOptions {
            level,
            icc_profile: "perfil.icc".into(),
            condition_identifier: "FOGRA39".into(),
            condition: "Papel estucado (ISO 12647-2)".into(),
            title: Some("Folleto (v2)".into()),
            ghostscript: None,
        }
    }

    #[test]
    fn escapes_postscript_strings() {
        assert_eq!(ps_string(r"a(b)\c"), r"(a\(b\)\\c)");
    }

    #[test]
    fn definition_declares_version_and_intent() {
        let d = pdfx_definition(&opts(PdfxLevel::X1a), "/tmp/p.icc");
        assert!(d.contains("/GTS_PDFXVersion (PDF/X-1a:2001) /GTS_PDFXConformance (PDF/X-1a:2001)"));
        assert!(d.contains("/OutputConditionIdentifier (FOGRA39)"));
        assert!(d.contains("/Title (Folleto \\(v2\\))"));
        assert!(d.contains("(/tmp/p.icc) (r) file"));
        let x4 = pdfx_definition(&opts(PdfxLevel::X4), "/tmp/p.icc");
        assert!(x4.contains("/GTS_PDFXVersion (PDF/X-4) /Title"));
        assert!(!x4.contains("Conformance"));
    }
}
