//! Automatización COM de Microsoft Office en Windows.
//!
//! Se hace a través de `powershell.exe` (`New-Object -ComObject`), así que no hay código `unsafe` ni dependencias
//! nuevas. Las rutas viajan en variables de entorno para no depender del entrecomillado.

use std::path::Path;
use std::process::Command;

use pliego_core::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum App {
    Word,
    Excel,
    PowerPoint,
}

impl App {
    pub fn prog_id(self) -> &'static str {
        match self {
            App::Word => "Word.Application",
            App::Excel => "Excel.Application",
            App::PowerPoint => "PowerPoint.Application",
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            App::Word => "Microsoft Word",
            App::Excel => "Microsoft Excel",
            App::PowerPoint => "Microsoft PowerPoint",
        }
    }
}

fn powershell() -> Command {
    let mut c = Command::new("powershell.exe");
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass"]);
    c
}

/// Aplicaciones de Office registradas en el sistema (consulta el registro; no las inicia).
pub fn installed(app: App) -> bool {
    Command::new("reg")
        .args(["query", &format!("HKCR\\{}", app.prog_id())])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn script(app: App) -> &'static str {
    match app {
        App::Word => {
            "$ErrorActionPreference='Stop'; $a=New-Object -ComObject Word.Application; $a.Visible=$false; $a.DisplayAlerts=0; \
             try { $d=$a.Documents.Open($env:PLIEGO_IN,$false,$true); try { $d.ExportAsFixedFormat($env:PLIEGO_OUT,17) } finally { $d.Close($false) } } \
             finally { $a.Quit() }"
        }
        App::Excel => {
            "$ErrorActionPreference='Stop'; $a=New-Object -ComObject Excel.Application; $a.Visible=$false; $a.DisplayAlerts=$false; \
             try { $w=$a.Workbooks.Open($env:PLIEGO_IN,0,$true); try { $w.ExportAsFixedFormat(0,$env:PLIEGO_OUT) } finally { $w.Close($false) } } \
             finally { $a.Quit() }"
        }
        App::PowerPoint => {
            "$ErrorActionPreference='Stop'; $a=New-Object -ComObject PowerPoint.Application; \
             try { $p=$a.Presentations.Open($env:PLIEGO_IN,-1,0,0); try { $p.SaveAs($env:PLIEGO_OUT,32) } finally { $p.Close() } } \
             finally { $a.Quit() }"
        }
    }
}

/// Convierte `input` en `output` (PDF) con la aplicación indicada.
pub fn convert(app: App, input: &Path, output: &Path) -> Result<()> {
    let out = powershell()
        .args(["-Command", script(app)])
        .env("PLIEGO_IN", input)
        .env("PLIEGO_OUT", output)
        .output()
        .map_err(|e| Error::Other(format!("no se pudo iniciar PowerShell: {e}")))?;
    if !out.status.success() || !output.exists() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("error desconocido");
        return Err(Error::Other(format!(
            "{} no pudo convertir {}: {first}",
            app.display(),
            input.display()
        )));
    }
    Ok(())
}
