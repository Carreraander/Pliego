//! Validación completa opcional con veraPDF (descarga aparte; necesita Java). Pliego no lo incluye:
//! si está instalado, lo ejecuta y traduce su informe a un resumen legible.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pliego_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::Level;

/// Variable de entorno para indicar dónde está `verapdf` (ejecutable o `.bat`).
pub const ENV_VAR: &str = "PLIEGO_VERAPDF";

/// Un grupo de comprobaciones fallidas (una regla de la norma).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VeraFailure {
    pub specification: String,
    pub clause: String,
    pub test_number: String,
    pub failed_checks: u32,
    pub description: String,
}

/// Resultado de veraPDF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VeraReport {
    pub compliant: bool,
    pub failures: Vec<VeraFailure>,
}

fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Busca veraPDF: variable de entorno, PATH y carpetas de instalación habituales.
pub fn find() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(ENV_VAR)
        .map(PathBuf::from)
        .filter(|p| is_executable(p))
    {
        return Some(p);
    }
    let names: &[&str] = if cfg!(windows) {
        &["verapdf.bat", "verapdf.exe"]
    } else {
        &["verapdf"]
    };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for n in names {
                let c = dir.join(n);
                if is_executable(&c) {
                    return Some(c);
                }
            }
        }
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(v) = std::env::var_os(var) {
                roots.push(PathBuf::from(v).join("veraPDF"));
            }
        }
        if let Some(v) = std::env::var_os("USERPROFILE") {
            roots.push(PathBuf::from(v).join("veraPDF"));
        }
    } else if let Some(h) = std::env::var_os("HOME") {
        let home = PathBuf::from(h);
        roots.push(home.join("verapdf"));
        roots.push(home.join("veraPDF"));
    }
    for r in roots {
        for n in names {
            let c = r.join(n);
            if is_executable(&c) {
                return Some(c);
            }
        }
    }
    None
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!(" {name}=\"");
    let i = tag.find(&pat)? + pat.len();
    let j = tag[i..].find('"')?;
    Some(unescape(&tag[i..i + j]))
}

/// Traduce el XML de veraPDF a un [`VeraReport`]. Devuelve `None` si no parece un informe.
pub fn parse_report(xml: &str) -> Option<VeraReport> {
    let start = xml.find("<validationReport")?;
    let head_end = xml[start..].find('>')? + start;
    let compliant = attr(&xml[start..head_end], "isCompliant")? == "true";
    let mut failures = Vec::new();
    let mut rest = &xml[head_end..];
    while let Some(i) = rest.find("<rule ") {
        rest = &rest[i..];
        let end = rest.find('>')?;
        let tag = &rest[..end];
        let body_end = rest.find("</rule>").unwrap_or(rest.len());
        if attr(tag, "status").as_deref() == Some("failed") {
            let body = &rest[end..body_end];
            let description = body
                .find("<description>")
                .and_then(|a| {
                    let a = a + "<description>".len();
                    body[a..]
                        .find("</description>")
                        .map(|b| unescape(body[a..a + b].trim()))
                })
                .unwrap_or_default();
            failures.push(VeraFailure {
                specification: attr(tag, "specification").unwrap_or_default(),
                clause: attr(tag, "clause").unwrap_or_default(),
                test_number: attr(tag, "testNumber").unwrap_or_default(),
                failed_checks: attr(tag, "failedChecks").and_then(|v| v.parse().ok()).unwrap_or(1),
                description,
            });
        }
        rest = &rest[body_end.min(rest.len())..];
        if rest.starts_with("</rule>") {
            rest = &rest["</rule>".len()..];
        }
    }
    Some(VeraReport { compliant, failures })
}

fn flavour(level: Level) -> &'static str {
    match level {
        Level::A1b => "1b",
        Level::A2b => "2b",
        Level::A2u => "2u",
        Level::A3b => "3b",
    }
}

/// Ejecuta veraPDF (`exe`) sobre `input` con un límite de tiempo.
pub fn run_with(exe: &Path, input: &Path, level: Level, timeout: Duration) -> Result<VeraReport> {
    let mut child = Command::new(exe)
        .args(["--flavour", flavour(level), "--format", "xml"])
        .arg(input)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Other(format!("no se pudo ejecutar veraPDF ({}): {e}", exe.display())))?;
    // Se leen las salidas en hilos para que un informe grande no bloquee el proceso.
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let t_out = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = out.as_mut() {
            let _ = std::io::Read::read_to_string(o, &mut s);
        }
        s
    });
    let t_err = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = err.as_mut() {
            let _ = std::io::Read::read_to_string(o, &mut s);
        }
        s
    });
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait()? {
            Some(_) => break,
            None if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Other("veraPDF tardó demasiado y se canceló".into()));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let stdout = t_out.join().unwrap_or_default();
    let stderr = t_err.join().unwrap_or_default();
    parse_report(&stdout).ok_or_else(|| {
        let tail: String = stderr
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" ");
        Error::Other(format!(
            "veraPDF no devolvió un informe válido{}",
            if tail.is_empty() {
                String::new()
            } else {
                format!(": {tail}")
            }
        ))
    })
}

/// Valida con veraPDF si está instalado; si no, explica cómo conseguirlo.
pub fn validate(input: &Path, level: Level) -> Result<VeraReport> {
    let exe = find().ok_or_else(|| {
        Error::Unsupported(format!(
            "veraPDF no está instalado (necesita Java). Descárgalo de https://verapdf.org/software/ o indica su ruta en la variable {ENV_VAR}"
        ))
    })?;
    run_with(&exe, input, level, Duration::from_secs(600))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<report><jobs><job><validationReport profileName="PDFA-2B validation profile" statement="x" isCompliant="false">
<details passedRules="100" failedRules="2" passedChecks="500" failedChecks="3">
<rule specification="ISO 19005-2:2011" clause="6.2.4.3" testNumber="2" status="failed" passedChecks="0" failedChecks="2">
<description>DeviceRGB &amp; friends are used without an &quot;OutputIntent&quot;</description>
<object>PDDeviceRGB</object><check status="failed"><context>root</context></check></rule>
<rule specification="ISO 19005-2:2011" clause="6.1.2" testNumber="1" status="passed" passedChecks="1" failedChecks="0">
<description>ok</description></rule>
<rule specification="ISO 19005-2:2011" clause="6.3.4" testNumber="1" status="failed" passedChecks="0" failedChecks="1">
<description>Font not embedded</description></rule>
</details></validationReport></job></jobs></report>"#;

    #[test]
    fn parses_failures() {
        let r = parse_report(SAMPLE).unwrap();
        assert!(!r.compliant);
        assert_eq!(r.failures.len(), 2);
        assert_eq!(r.failures[0].clause, "6.2.4.3");
        assert_eq!(r.failures[0].failed_checks, 2);
        assert_eq!(
            r.failures[0].description,
            "DeviceRGB & friends are used without an \"OutputIntent\""
        );
        assert_eq!(r.failures[1].description, "Font not embedded");
    }

    #[test]
    fn parses_compliant_and_rejects_garbage() {
        let ok = r#"<validationReport isCompliant="true"><details failedRules="0"></details></validationReport>"#;
        assert_eq!(
            parse_report(ok),
            Some(VeraReport {
                compliant: true,
                failures: vec![]
            })
        );
        assert!(parse_report("Exception in thread main").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn runs_an_external_program() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("verapdf");
        std::fs::write(&script, format!("#!/bin/sh\ncat <<'EOF'\n{SAMPLE}\nEOF\nexit 1\n")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let r = run_with(&script, Path::new("x.pdf"), Level::A2b, Duration::from_secs(20)).unwrap();
        assert_eq!(r.failures.len(), 2);
        // y un programa que falla sin informe produce un error legible
        let bad = dir.path().join("bad");
        std::fs::write(&bad, "#!/bin/sh\necho 'Java no encontrado' >&2\nexit 2\n").unwrap();
        std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o755)).unwrap();
        let e = run_with(&bad, Path::new("x.pdf"), Level::A2b, Duration::from_secs(20)).unwrap_err();
        assert!(e.to_string().contains("Java no encontrado"), "{e}");
    }

    #[test]
    fn missing_program_is_an_error() {
        assert!(
            run_with(
                Path::new("/no/existe/verapdf"),
                Path::new("x.pdf"),
                Level::A1b,
                Duration::from_secs(1)
            )
            .is_err()
        );
    }
}
