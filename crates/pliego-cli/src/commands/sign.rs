use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Args, ValueEnum};
use pliego_core::JobContext;
use pliego_sign::{ChainStatus, PadesLevel, Pkcs12Signer, SignOptions, ValidationOptions};

#[derive(Clone, Copy, ValueEnum)]
pub enum Level {
    #[value(name = "b-b")]
    BB,
    #[value(name = "b-t")]
    BT,
    #[value(name = "b-lt")]
    BLt,
}

#[derive(Args)]
pub struct SignArgs {
    /// PDF de entrada (no se modifica).
    pub input: PathBuf,
    /// PDF firmado de salida.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Certificado y clave privada en un archivo PKCS#12 (.pfx/.p12).
    #[arg(long)]
    pub p12: PathBuf,
    /// Contraseña del archivo PKCS#12 (también en la variable PLIEGO_P12_PASSWORD).
    #[arg(long)]
    pub password: Option<String>,
    /// Página (desde 1) donde se coloca el campo de firma.
    #[arg(long, default_value_t = 1)]
    pub page: usize,
    /// Nivel PAdES: b-b (básico), b-t (con sello de tiempo, requiere --tsa) o b-lt (además, con certificados y CRL incrustados).
    #[arg(long, value_enum, default_value = "b-b")]
    pub level: Level,
    /// Dirección http del servidor de sellado de tiempo (RFC 3161).
    #[arg(long)]
    pub tsa: Option<String>,
    /// Lista CRL (DER) para b-lt; se puede repetir. Sin ella se descargan de los certificados.
    #[arg(long = "crl")]
    pub crl: Vec<PathBuf>,
    /// Nombre del campo de firma.
    #[arg(long)]
    pub field: Option<String>,
    #[arg(long)]
    pub reason: Option<String>,
    #[arg(long)]
    pub location: Option<String>,
    #[arg(long)]
    pub contact: Option<String>,
}

#[derive(Args)]
pub struct VerifyArgs {
    /// PDF a comprobar.
    pub input: PathBuf,
    /// Certificado raíz de confianza (PEM o DER); se puede repetir.
    #[arg(long = "trust")]
    pub trust: Vec<PathBuf>,
    /// Falla también si hay cambios posteriores o la cadena no es de confianza.
    #[arg(long)]
    pub strict: bool,
}

pub fn sign(a: SignArgs, json: bool) -> anyhow::Result<()> {
    if a.page == 0 {
        bail!("las páginas se numeran desde 1");
    }
    let password = a
        .password
        .or_else(|| std::env::var("PLIEGO_P12_PASSWORD").ok())
        .unwrap_or_default();
    let signer = Pkcs12Signer::from_file(&a.p12, &password)?;
    let opts = SignOptions {
        level: match a.level {
            Level::BB => PadesLevel::BB,
            Level::BT => PadesLevel::BT,
            Level::BLt => PadesLevel::BLt,
        },
        timestamp_url: a.tsa,
        crl_files: a.crl,
        page: a.page - 1,
        field_name: a.field,
        reason: a.reason,
        location: a.location,
        contact: a.contact,
        ..Default::default()
    };
    let report = pliego_sign::sign(&a.input, &a.output, &signer, opts, &JobContext::silent())
        .with_context(|| format!("no se pudo firmar {}", a.input.display()))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}: {}", report.summary, a.output.display());
    }
    Ok(())
}

fn level_text(l: PadesLevel) -> &'static str {
    match l {
        PadesLevel::BB => "B-B",
        PadesLevel::BT => "B-T (con sello de tiempo)",
        PadesLevel::BLt => "B-LT (validación a largo plazo)",
        PadesLevel::BLta => "B-LTA",
    }
}

fn chain_text(c: ChainStatus) -> &'static str {
    match c {
        ChainStatus::Trusted => "de confianza",
        ChainStatus::UntrustedRoot => "raíz sin confianza",
        ChainStatus::Incomplete => "cadena incompleta",
        ChainStatus::Invalid => "cadena no válida",
    }
}

pub fn verify(a: VerifyArgs, json: bool) -> anyhow::Result<()> {
    let trust_anchors = a
        .trust
        .iter()
        .map(pliego_sign::load_certificate)
        .collect::<Result<Vec<_>, _>>()?;
    let reports = pliego_sign::validate_with(&a.input, &ValidationOptions { trust_anchors })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else if reports.is_empty() {
        println!("El documento no contiene firmas.");
    }
    if !json {
        for (i, r) in reports.iter().enumerate() {
            let state = if r.is_valid() {
                "VÁLIDA"
            } else if r.integrity {
                "ÍNTEGRA con avisos"
            } else {
                "ROTA"
            };
            println!("Firma {}: {} · {}", i + 1, state, r.signer);
            if let Some(f) = &r.field {
                println!("  campo: {f}");
            }
            if let Some(t) = &r.signing_time {
                println!("  fecha: {t}");
            }
            println!("  certificado: {}", chain_text(r.chain));
            println!("  nivel: {}", level_text(r.level));
            if let Some(t) = &r.timestamp {
                println!(
                    "  sello de tiempo: {} ({})",
                    t.time,
                    if t.valid { "válido" } else { "no válido" }
                );
            }
            for p in &r.problems {
                println!("  aviso: {p}");
            }
        }
    }
    let failed = reports.is_empty() || reports.iter().any(|r| !r.integrity || (a.strict && !r.is_valid()));
    if failed {
        std::process::exit(1);
    }
    Ok(())
}
