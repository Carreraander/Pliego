use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Args;
use pliego_core::JobContext;
use pliego_sign::{ChainStatus, Pkcs12Signer, SignOptions, ValidationOptions};

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
