use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use pliego_sign::{Pkcs12Signer, SignOptions};
use pliego_signflow::{AuditEvent, SignerSpec, Signers};

#[derive(Args)]
pub struct SignflowArgs {
    #[command(subcommand)]
    pub action: Action,
}

#[derive(Subcommand)]
pub enum Action {
    /// Prepara un PDF para que lo firmen varias personas en orden.
    Prepare {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// Firmante: «Nombre» o «Nombre<correo>»; se repite, en el orden de firma.
        #[arg(long = "signer", required = true)]
        signers: Vec<String>,
    },
    /// Indica a quién le toca firmar.
    Next { input: PathBuf },
    /// Añade la firma del siguiente firmante.
    Sign {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        p12: PathBuf,
        /// Contraseña del PKCS#12 (también en PLIEGO_P12_PASSWORD).
        #[arg(long)]
        password: Option<String>,
    },
    /// Muestra el registro de auditoría.
    Audit { input: PathBuf },
}

fn parse_signer(s: &str) -> SignerSpec {
    match s.split_once('<') {
        Some((name, mail)) => SignerSpec {
            email: Some(mail.trim_end_matches('>').trim().to_string()),
            ..SignerSpec::new(name.trim())
        },
        None => SignerSpec::new(s.trim()),
    }
}

fn write_new(path: &Path, data: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_extension("pliego-tmp");
    std::fs::write(&tmp, data).with_context(|| format!("no se pudo escribir {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("no se pudo crear {}", path.display()))?;
    Ok(())
}

fn read(path: &Path) -> anyhow::Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("no se pudo leer {}", path.display()))
}

pub fn run(a: SignflowArgs, json: bool) -> anyhow::Result<()> {
    match a.action {
        Action::Prepare { input, output, signers } => {
            if input == output {
                bail!("el archivo de salida no puede ser el de entrada");
            }
            let list = Signers::new(signers.iter().map(|s| parse_signer(s)).collect());
            let pkg = pliego_signflow::prepare(&read(&input)?, &list)?;
            write_new(&output, &pkg.pdf)?;
            println!(
                "Paquete preparado para {} firmantes: {}",
                list.0.len(),
                output.display()
            );
        }
        Action::Next { input } => {
            let next = pliego_signflow::next_signer(&read(&input)?)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&next)?);
            } else if let Some(p) = next {
                println!("Le toca firmar a {} ({} de {})", p.spec.name, p.index + 1, p.total);
            } else {
                println!("Ya han firmado todos.");
            }
        }
        Action::Sign {
            input,
            output,
            p12,
            password,
        } => {
            if input == output {
                bail!("el archivo de salida no puede ser el de entrada");
            }
            let password = password
                .or_else(|| std::env::var("PLIEGO_P12_PASSWORD").ok())
                .unwrap_or_default();
            let signer = Pkcs12Signer::from_file(&p12, &password)?;
            let (pdf, after) = pliego_signflow::sign_next(&read(&input)?, &signer, SignOptions::default())?;
            write_new(&output, &pdf)?;
            match after {
                Some(p) => println!("Firmado. Siguiente: {} ({} de {})", p.spec.name, p.index + 1, p.total),
                None => println!("Firmado. Ya han firmado todos."),
            }
        }
        Action::Audit { input } => {
            let log = pliego_signflow::audit_log(&read(&input)?)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&log)?);
            }
            for e in log.iter().filter(|_| !json) {
                let when = e.time.as_deref().unwrap_or("sin fecha");
                match &e.event {
                    AuditEvent::Prepared { signers, .. } => println!("{when}  Preparado para: {}", signers.join(", ")),
                    AuditEvent::Signed {
                        certificate,
                        in_order,
                        integrity,
                        document_sha256,
                        ..
                    } => println!(
                        "{when}  Firma de {certificate}{}{}  huella {}",
                        if *in_order { "" } else { " (fuera de orden)" },
                        if *integrity { "" } else { " (ROTA)" },
                        &document_sha256[..16]
                    ),
                }
            }
        }
    }
    Ok(())
}
