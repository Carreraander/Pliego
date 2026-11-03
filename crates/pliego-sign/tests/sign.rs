mod common;

use common::*;
use pliego_sign::{ChainStatus, Pkcs12Signer, SignOptions, ValidationOptions, sign_bytes, validate_bytes};
use x509_cert::Certificate;
use x509_cert::der::Decode;

fn anchors(pki: &TestPki) -> ValidationOptions {
    ValidationOptions {
        trust_anchors: vec![Certificate::from_der(&pki.ca_der).unwrap()],
    }
}

fn roundtrip(ecdsa: bool) {
    let pki = make_pki(ecdsa);
    let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
    let opts = SignOptions {
        reason: Some("Aprobado".into()),
        location: Some("Sevilla".into()),
        ..Default::default()
    };
    let signed = sign_bytes(&corpus_pdf(), &signer, &opts).unwrap();
    if let Ok(dir) = std::env::var("PLIEGO_SIGN_OUT") {
        std::fs::write(
            format!("{dir}/firmado-{}.pdf", if ecdsa { "ecdsa" } else { "rsa" }),
            &signed,
        )
        .unwrap();
        std::fs::write(
            format!("{dir}/ca-{}.der", if ecdsa { "ecdsa" } else { "rsa" }),
            &pki.ca_der,
        )
        .unwrap();
    }
    let reports = validate_bytes(&signed, &anchors(&pki)).unwrap();
    assert_eq!(reports.len(), 1);
    let r = &reports[0];
    assert!(r.integrity, "{:?}", r.problems);
    assert!(r.covers_whole_document);
    assert_eq!(r.chain, ChainStatus::Trusted);
    assert!(r.is_valid());
    assert_eq!(r.signer, "Persona de Prueba");
    assert_eq!(r.reason.as_deref(), Some("Aprobado"));
    // Sin la CA configurada la raíz no es de confianza.
    let r = &validate_bytes(&signed, &ValidationOptions::default()).unwrap()[0];
    assert!(r.integrity);
    assert_eq!(r.chain, ChainStatus::UntrustedRoot);
}

#[test]
fn sign_and_validate_rsa() {
    roundtrip(false);
}

#[test]
fn sign_and_validate_ecdsa() {
    roundtrip(true);
}

#[test]
fn tampering_breaks_signature() {
    let pki = make_pki(false);
    let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
    let mut signed = sign_bytes(&corpus_pdf(), &signer, &SignOptions::default()).unwrap();
    let pos = signed.windows(5).position(|w| w == b"Hola ").unwrap();
    signed[pos] = b'h';
    let r = &validate_bytes(&signed, &anchors(&pki)).unwrap()[0];
    assert!(!r.integrity);
    assert!(!r.is_valid());
    assert!(r.problems.iter().any(|p| p.contains("modificado")), "{:?}", r.problems);
}

#[test]
fn appended_changes_are_reported() {
    let pki = make_pki(false);
    let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
    let mut signed = sign_bytes(&corpus_pdf(), &signer, &SignOptions::default()).unwrap();
    signed.extend_from_slice(b"\n%basura\n");
    let r = &validate_bytes(&signed, &anchors(&pki)).unwrap()[0];
    assert!(r.integrity);
    assert!(!r.covers_whole_document);
}

#[test]
fn second_signature_keeps_first_valid() {
    let pki = make_pki(false);
    let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
    let once = sign_bytes(&corpus_pdf(), &signer, &SignOptions::default()).unwrap();
    let twice = sign_bytes(&once, &signer, &SignOptions::default()).unwrap();
    let reports = validate_bytes(&twice, &anchors(&pki)).unwrap();
    assert_eq!(reports.len(), 2);
    assert!(reports.iter().all(|r| r.integrity), "{reports:?}");
    assert_eq!(reports.iter().filter(|r| r.covers_whole_document).count(), 1);
}

#[test]
fn wrong_password_is_a_clear_error() {
    let pki = make_pki(false);
    let err = Pkcs12Signer::from_bytes(&pki.p12, "otra").err().unwrap();
    assert!(err.to_string().contains("contraseña"));
}
