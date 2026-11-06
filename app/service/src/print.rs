//! Imprimir desde la app (W45): `pliego printer …` como **proceso hijo**, igual que el OCR.
//!
//! La app no dibuja páginas en la impresora: lo hace la CLI `pliego` que va en su carpeta (con PDFium), de modo que un
//! PDF que tumbe a PDFium o un controlador de impresora que se cuelgue no cierran la interfaz. Esta capa arma los
//! argumentos, lee el progreso (`[hecho/total] mensaje` por la salida de error) y el informe JSON, y cancela matando al
//! hijo. Las consultas del diálogo (impresoras, papeles, zona imprimible) también van por el hijo: si un controlador de
//! red no contesta, se puede esperar con un tiempo límite sin bloquear la app.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
pub use pliego_printer::{
    Booklet, Duplex, NupOrder, Orientation, Placement, PrintOptions, PrinterCaps, PrinterEntry, ScaleMode,
    SheetMetrics, SheetPlan, Subset,
};
use serde::{Deserialize, Serialize};

use crate::ocr::{run_cli, run_cli_with_password};

/// Lo que pide el diálogo de impresión.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PrintRequest {
    pub path: PathBuf,
    /// Contraseña del PDF si está protegido. Va al hijo por `PLIEGO_PASSWORD`, nunca por la línea de órdenes.
    pub password: Option<String>,
    pub options: PrintOptions,
}

impl Default for PrintRequest {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            password: None,
            options: PrintOptions::default(),
        }
    }
}

/// Argumentos de `pliego printer send` para una petición.
pub fn send_args(req: &PrintRequest) -> Result<Vec<OsString>> {
    let o = &req.options;
    o.validate()?;
    let mut a: Vec<OsString> = vec![
        "--json".into(),
        "printer".into(),
        "send".into(),
        req.path.clone().into_os_string(),
    ];
    let mut flag = |name: &str, value: String| {
        a.push(name.into());
        a.push(value.into());
    };
    if let Some(p) = &o.printer {
        flag("--printer", p.clone());
    }
    flag("--pages", o.pages.clone());
    flag(
        "--subset",
        match o.subset {
            Subset::All => "all",
            Subset::Odd => "odd",
            Subset::Even => "even",
        }
        .into(),
    );
    flag("--copies", o.copies.to_string());
    flag(
        "--orientation",
        match o.orientation {
            Orientation::Auto => "auto",
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
        .into(),
    );
    flag(
        "--scale",
        match o.scale {
            ScaleMode::Fit => "fit",
            ScaleMode::Shrink => "shrink",
            ScaleMode::Actual => "actual",
            ScaleMode::Custom => "custom",
        }
        .into(),
    );
    flag("--scale-percent", o.scale_percent.to_string());
    flag(
        "--duplex",
        match o.duplex {
            Duplex::Off => "off",
            Duplex::LongEdge => "long",
            Duplex::ShortEdge => "short",
        }
        .into(),
    );
    if let Some(p) = o.paper {
        flag("--paper", p.to_string());
    }
    flag("--per-sheet", o.pages_per_sheet.to_string());
    flag(
        "--nup-order",
        match o.nup_order {
            NupOrder::Horizontal => "horizontal",
            NupOrder::HorizontalReversed => "horizontal-reversed",
            NupOrder::Vertical => "vertical",
            NupOrder::VerticalReversed => "vertical-reversed",
        }
        .into(),
    );
    flag(
        "--booklet",
        match o.booklet {
            Booklet::Off => "off",
            Booklet::Both => "both",
            Booklet::FrontOnly => "front",
            Booklet::BackOnly => "back",
        }
        .into(),
    );
    if let Some(out) = &o.output {
        a.push("--output".into());
        a.push(out.clone().into_os_string());
    }
    for (on, name) in [
        (o.reverse, "--reverse"),
        (!o.collate, "--no-collate"),
        (!o.auto_rotate, "--no-auto-rotate"),
        (o.grayscale, "--grayscale"),
        (!o.comments, "--no-comments"),
        (o.borders, "--borders"),
        (o.booklet_rtl, "--booklet-rtl"),
    ] {
        if on {
            a.push(name.into());
        }
    }
    Ok(a)
}

/// Imprime el PDF de `req` y devuelve el informe de la CLI.
pub fn print_document(cli: &Path, req: &PrintRequest, ctx: &JobContext) -> Result<Report> {
    let args = send_args(req)?;
    let stdout = run_cli_with_password(cli, &args, req.password.as_deref(), ctx)?;
    serde_json::from_str(stdout.trim()).map_err(|e| Error::Other(format!("respuesta de la impresión no válida: {e}")))
}

fn query<T: serde::de::DeserializeOwned>(cli: &Path, args: Vec<OsString>) -> Result<T> {
    let out = run_cli(cli, &args, &JobContext::silent())?;
    serde_json::from_str(out.trim()).map_err(|e| Error::Other(format!("respuesta de la impresora no válida: {e}")))
}

/// Impresoras instaladas.
pub fn printers(cli: &Path) -> Result<Vec<PrinterEntry>> {
    query(cli, vec!["--json".into(), "printer".into(), "list".into()])
}

/// Papeles y capacidades de una impresora.
pub fn printer_caps(cli: &Path, name: &str) -> Result<PrinterCaps> {
    query(cli, vec!["--json".into(), "printer".into(), "caps".into(), name.into()])
}

/// Hoja de una impresora en puntos (para la vista previa).
pub fn sheet(cli: &Path, name: &str, paper: Option<i16>, landscape: bool) -> Result<SheetMetrics> {
    let mut args: Vec<OsString> = vec!["--json".into(), "printer".into(), "sheet".into(), name.into()];
    if let Some(p) = paper {
        args.push("--paper".into());
        args.push(p.to_string().into());
    }
    if landscape {
        args.push("--landscape".into());
    }
    query(cli, args)
}

/// Cómo sale la primera cara en la hoja de la vista previa y cuántas caras tiene el trabajo (mismas cuentas que la
/// impresión real).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPlan {
    /// Caras distintas que se imprimen (sin contar las copias).
    pub sheets: u32,
    pub first: SheetPlan,
}

/// Plan de la vista previa. `sizes[i]` es el tamaño en puntos de la página `i` del documento.
pub fn preview_plan(sizes: &[(f32, f32)], sheet: &SheetMetrics, options: &PrintOptions) -> Result<PreviewPlan> {
    options.validate()?;
    let mut plans = pliego_printer::plan_sheets(sizes, sheet, options)?;
    let sheets = plans.len() as u32;
    Ok(PreviewPlan {
        sheets,
        first: plans.swap_remove(0),
    })
}

/// ¿La hoja es apaisada con estas opciones? Para pedir la hoja de la impresora en la orientación que usará el trabajo.
pub fn preview_landscape(options: &PrintOptions, first_page: (f32, f32)) -> Result<bool> {
    options.validate()?;
    Ok(pliego_printer::choose_landscape(options, first_page))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fake_cli(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("pliego");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn args_of(req: &PrintRequest) -> Vec<String> {
        send_args(req)
            .unwrap()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn builds_the_send_arguments() {
        let req = PrintRequest {
            path: "a.pdf".into(),
            options: PrintOptions {
                printer: Some("Mi impresora".into()),
                pages: "1-3".into(),
                copies: 2,
                collate: false,
                scale: ScaleMode::Custom,
                scale_percent: 80,
                duplex: Duplex::LongEdge,
                grayscale: true,
                comments: false,
                paper: Some(9),
                ..PrintOptions::default()
            },
            ..PrintRequest::default()
        };
        let a = args_of(&req);
        assert_eq!(a[..4], ["--json", "printer", "send", "a.pdf"]);
        let pair = |k: &str, v: &str| a.windows(2).any(|w| w[0] == k && w[1] == v);
        assert!(pair("--printer", "Mi impresora") && pair("--pages", "1-3") && pair("--copies", "2"));
        assert!(pair("--scale", "custom") && pair("--scale-percent", "80") && pair("--duplex", "long"));
        assert!(pair("--paper", "9"));
        for f in ["--no-collate", "--grayscale", "--no-comments"] {
            assert!(a.contains(&f.to_string()), "falta {f}");
        }
        assert!(!a.contains(&"--reverse".to_string()) && !a.contains(&"--no-auto-rotate".to_string()));
        // Una opción no válida no llega a lanzar nada.
        let bad = PrintRequest {
            options: PrintOptions {
                copies: 0,
                ..PrintOptions::default()
            },
            ..req
        };
        assert!(send_args(&bad).is_err());
    }

    #[test]
    fn prints_through_the_cli_with_progress_and_password_in_the_environment() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(
            dir.path(),
            r#"echo "[1/1] Imprimiendo la página 1" >&2
case "$*" in *s3creta*) leak=si;; *) leak=no;; esac
echo "{\"outputs\":[],\"warnings\":[\"aviso\"],\"summary\":\"pw=$PLIEGO_PASSWORD fuga=$leak\"}""#,
        );
        let req = PrintRequest {
            path: "a.pdf".into(),
            password: Some("s3creta".into()),
            ..PrintRequest::default()
        };
        let r = print_document(&cli, &req, &JobContext::silent()).unwrap();
        assert_eq!(r.summary, "pw=s3creta fuga=no");
        assert_eq!(r.warnings, ["aviso"]);
    }

    #[test]
    fn a_failing_print_returns_the_clis_message() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(
            dir.path(),
            r#"echo "Error: la impresora no aceptó el documento" >&2; exit 1"#,
        );
        let e = print_document(&cli, &PrintRequest::default(), &JobContext::silent()).unwrap_err();
        assert!(e.to_string().contains("la impresora no aceptó"), "{e}");
    }

    #[test]
    fn queries_parse_the_json_of_the_cli() {
        let _serial = crate::test_support::serial();
        let dir = tempfile::tempdir().unwrap();
        let cli = fake_cli(
            dir.path(),
            r#"case "$*" in
*"printer list"*) echo '[{"name":"HP","isDefault":true},{"name":"PDF","isDefault":false}]';;
*"printer caps"*) echo '{"name":"HP","color":true,"duplex":true,"maxCopies":99,"collate":true,"papers":[{"id":9,"name":"A4","widthMm":210.0,"heightMm":297.0}],"defaultPaper":9}';;
*"printer sheet"*) echo '{"dpiX":72.0,"dpiY":72.0,"physW":595.0,"physH":842.0,"offX":10.0,"offY":12.0,"areaW":575.0,"areaH":818.0}';;
esac"#,
        );
        let l = printers(&cli).unwrap();
        assert_eq!(l.len(), 2);
        assert!(l[0].is_default);
        let c = printer_caps(&cli, "HP").unwrap();
        assert!(c.duplex && c.max_copies == 99 && c.papers[0].id == 9);
        let s = sheet(&cli, "HP", Some(9), true).unwrap();
        assert_eq!((s.phys_w, s.off_x), (595.0, 10.0));
    }

    #[test]
    fn preview_uses_the_same_layout_as_printing() {
        let s = SheetMetrics::plain(595.0, 842.0);
        let sizes = [(842.0, 595.0)];
        let p = preview_plan(&sizes, &s, &PrintOptions::default()).unwrap();
        assert_eq!((p.sheets, p.first.slots[0].placement.rotate), (1, 1));
        assert!(
            preview_plan(
                &[(10.0, 10.0)],
                &s,
                &PrintOptions {
                    copies: 0,
                    ..PrintOptions::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn preview_of_a_booklet_counts_sides_and_picks_landscape() {
        let o = PrintOptions {
            booklet: Booklet::Both,
            ..PrintOptions::default()
        };
        assert!(preview_landscape(&o, (595.0, 842.0)).unwrap());
        let s = SheetMetrics::plain(842.0, 595.0);
        let p = preview_plan(&[(595.0, 842.0); 8], &s, &o).unwrap();
        assert_eq!(p.sheets, 4);
        assert_eq!(p.first.slots.len(), 2);
    }

    #[test]
    fn sheet_layout_flags_reach_the_cli() {
        let req = PrintRequest {
            path: "a.pdf".into(),
            options: PrintOptions {
                pages_per_sheet: 4,
                nup_order: NupOrder::VerticalReversed,
                borders: true,
                ..PrintOptions::default()
            },
            ..PrintRequest::default()
        };
        let a = args_of(&req);
        let pair = |k: &str, v: &str| a.windows(2).any(|w| w[0] == k && w[1] == v);
        assert!(pair("--per-sheet", "4") && pair("--nup-order", "vertical-reversed") && pair("--booklet", "off"));
        assert!(a.contains(&"--borders".to_string()) && !a.contains(&"--booklet-rtl".to_string()));
        let book = PrintRequest {
            options: PrintOptions {
                booklet: Booklet::BackOnly,
                booklet_rtl: true,
                ..PrintOptions::default()
            },
            ..PrintRequest::default()
        };
        let b = args_of(&book);
        assert!(
            b.windows(2).any(|w| w[0] == "--booklet" && w[1] == "back") && b.contains(&"--booklet-rtl".to_string())
        );
    }
}
