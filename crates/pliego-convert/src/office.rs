//! Documentos de Office a PDF.
//!
//! En Windows se usa la automatización COM de Word, Excel y PowerPoint si están instalados. Si no, o en otros
//! sistemas, se recurre a LibreOffice en modo headless. Si no hay ninguno, el error lo explica.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pliego_core::{Error, JobContext, Report, Result};
use serde::{Deserialize, Serialize};

use crate::util::{ensure_not_same, path_utf8};

/// Programa que convertirá los documentos de Office.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OfficeBackend {
    /// Microsoft Office por COM (solo Windows); indica qué aplicaciones están instaladas.
    MicrosoftOffice { word: bool, excel: bool, powerpoint: bool },
    /// LibreOffice en modo headless; ruta del ejecutable `soffice`.
    LibreOffice { path: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Word,
    Excel,
    PowerPoint,
}

fn family(path: &Path) -> Result<Family> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "doc" | "docx" | "docm" | "dot" | "dotx" | "rtf" | "odt" | "wps" => Ok(Family::Word),
        "xls" | "xlsx" | "xlsm" | "xlsb" | "csv" | "ods" => Ok(Family::Excel),
        "ppt" | "pptx" | "pptm" | "pps" | "ppsx" | "odp" => Ok(Family::PowerPoint),
        _ => Err(Error::InvalidInput(format!(
            "{}: formato de Office no reconocido (admitidos: Word, Excel, PowerPoint y OpenDocument)",
            path.display()
        ))),
    }
}

/// Ruta de LibreOffice (`soffice`), si está instalado. `PLIEGO_LIBREOFFICE` fuerza una ruta concreta.
pub fn find_libreoffice() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PLIEGO_LIBREOFFICE").map(PathBuf::from)
        && p.is_file()
    {
        return Some(p);
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    for name in ["soffice", "libreoffice", "soffice.exe"] {
        if let Some(paths) = std::env::var_os("PATH") {
            candidates.extend(std::env::split_paths(&paths).map(|d| d.join(name)));
        }
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(var) {
            candidates.push(PathBuf::from(base).join("LibreOffice/program/soffice.exe"));
        }
    }
    candidates.push("/Applications/LibreOffice.app/Contents/MacOS/soffice".into());
    candidates.push("/usr/lib/libreoffice/program/soffice".into());
    candidates.into_iter().find(|p| p.is_file())
}

#[cfg(windows)]
fn ms_office() -> (bool, bool, bool) {
    use crate::platform::windows::{App, installed};
    (installed(App::Word), installed(App::Excel), installed(App::PowerPoint))
}

#[cfg(not(windows))]
fn ms_office() -> (bool, bool, bool) {
    (false, false, false)
}

/// Qué hay disponible para convertir Office a PDF: Microsoft Office si está, si no LibreOffice, si no `None`.
pub fn office_backend() -> Option<OfficeBackend> {
    let (word, excel, powerpoint) = ms_office();
    if word || excel || powerpoint {
        return Some(OfficeBackend::MicrosoftOffice {
            word,
            excel,
            powerpoint,
        });
    }
    find_libreoffice().map(|path| OfficeBackend::LibreOffice { path })
}

fn missing_backend_error() -> Error {
    Error::Unsupported(
        "para convertir documentos de Office a PDF hace falta Microsoft Office (Word, Excel o PowerPoint) o \
         LibreOffice; no se ha encontrado ninguno. Instala LibreOffice (gratuito) desde libreoffice.org"
            .into(),
    )
}

/// Convierte un documento de Office (Word, Excel, PowerPoint, OpenDocument, RTF, CSV) a PDF.
pub fn office_to_pdf(input: &Path, output: &Path, ctx: &JobContext) -> Result<Report> {
    ensure_not_same(input, output)?;
    if !input.is_file() {
        return Err(Error::io_at(input, std::io::Error::from(std::io::ErrorKind::NotFound)));
    }
    let fam = family(input)?;
    ctx.step(0, 2, "convirtiendo")?;
    let tmp_dir = tempfile::tempdir().map_err(Error::from)?;
    let tmp_pdf = tmp_dir.path().join("salida.pdf");

    let done = try_microsoft_office(fam, input, &tmp_pdf)?;
    if !done {
        let soffice = find_libreoffice().ok_or_else(missing_backend_error)?;
        libreoffice_convert(&soffice, input, tmp_dir.path(), &tmp_pdf, ctx)?;
    }

    ctx.step(1, 2, "guardando")?;
    if let Some(dir) = output.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| Error::io_at(dir, e))?;
    }
    let bytes = std::fs::read(&tmp_pdf).map_err(|e| Error::io_at(&tmp_pdf, e))?;
    crate::util::write_atomic(output, &bytes)?;
    ctx.step(2, 2, "listo")?;
    Ok(Report::with_output(output, "documento convertido a PDF"))
}

/// Intenta convertir con Microsoft Office; `false` si la aplicación necesaria no está instalada.
#[cfg(windows)]
fn try_microsoft_office(fam: Family, input: &Path, output: &Path) -> Result<bool> {
    use crate::platform::windows::{App, convert, installed};
    let app = match fam {
        Family::Word => App::Word,
        Family::Excel => App::Excel,
        Family::PowerPoint => App::PowerPoint,
    };
    if !installed(app) {
        return Ok(false);
    }
    let abs_in = std::fs::canonicalize(input).map_err(|e| Error::io_at(input, e))?;
    convert(app, &abs_in, output)?;
    Ok(true)
}

#[cfg(not(windows))]
fn try_microsoft_office(_fam: Family, _input: &Path, _output: &Path) -> Result<bool> {
    Ok(false)
}

/// Tiempo máximo para una conversión con LibreOffice.
const LIBREOFFICE_TIMEOUT: Duration = Duration::from_secs(600);

fn libreoffice_convert(soffice: &Path, input: &Path, work: &Path, expected: &Path, ctx: &JobContext) -> Result<()> {
    let outdir = work.join("lo-out");
    let profile = work.join("lo-profile");
    std::fs::create_dir_all(&outdir).map_err(|e| Error::io_at(&outdir, e))?;
    let profile_url = format!(
        "-env:UserInstallation=file://{}",
        path_utf8(&profile)?.replace('\\', "/")
    );
    // En Windows la URL necesita una barra extra: file:///C:/...
    let profile_url = if cfg!(windows) {
        profile_url.replacen("file://", "file:///", 1)
    } else {
        profile_url
    };
    let mut child = Command::new(soffice)
        .args([
            "--headless",
            "--norestore",
            "--nolockcheck",
            &profile_url,
            "--convert-to",
            "pdf",
            "--outdir",
        ])
        .arg(&outdir)
        .arg(input)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Other(format!("no se pudo iniciar LibreOffice ({}): {e}", soffice.display())))?;
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(Error::from)? {
            break s;
        }
        if ctx.cancel.is_cancelled() {
            let _ = child.kill();
            return Err(Error::Cancelled);
        }
        if start.elapsed() > LIBREOFFICE_TIMEOUT {
            let _ = child.kill();
            return Err(Error::Other(
                "LibreOffice tardó demasiado en convertir el documento".into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    // LibreOffice nombra la salida como la entrada con extensión .pdf.
    let produced = outdir.join(Path::new(input.file_name().unwrap_or_default()).with_extension("pdf"));
    if !status.success() || !produced.is_file() {
        return Err(Error::Other(format!(
            "LibreOffice no pudo convertir {} (¿archivo dañado o protegido con contraseña?)",
            input.display()
        )));
    }
    std::fs::rename(&produced, expected).map_err(|e| Error::io_at(&produced, e))?;
    Ok(())
}
