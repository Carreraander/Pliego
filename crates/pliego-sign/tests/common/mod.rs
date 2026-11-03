//! Utilidades de prueba: CA propia y certificados de firma generados al vuelo (nunca claves reales).
#![allow(dead_code)]

use std::path::PathBuf;

use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
use rcgen::{
    BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
    PKCS_RSA_SHA256,
};

pub struct TestPki {
    pub ca_der: Vec<u8>,
    pub p12: Vec<u8>,
    /// CRL vacía firmada por la CA.
    pub crl_der: Vec<u8>,
}

pub const PASSWORD: &str = "prueba";

/// CA + certificado de firma. `ecdsa` elige P-256 en vez de RSA-2048.
pub fn make_pki(ecdsa: bool) -> TestPki {
    make_pki_for(ecdsa, false)
}

/// Como `make_pki`; con `tsa` el certificado lleva el uso de clave de sellado de tiempo.
pub fn make_pki_for(ecdsa: bool, tsa: bool) -> TestPki {
    let alg = if ecdsa {
        &PKCS_ECDSA_P256_SHA256
    } else {
        &PKCS_RSA_SHA256
    };
    let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "CA de pruebas Pliego");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = Issuer::new(ca_params, ca_key);

    let key = if ecdsa {
        KeyPair::generate_for(alg).unwrap()
    } else {
        use rsa::pkcs8::EncodePrivateKey;
        let rsa_key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).unwrap();
        let der = rsa_key.to_pkcs8_der().unwrap();
        KeyPair::from_pkcs8_der_and_sign_algo(&der.as_bytes().into(), alg).unwrap()
    };
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.distinguished_name.push(DnType::CommonName, "Persona de Prueba");
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::ContentCommitment];
    if tsa {
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::TimeStamping];
    }
    let leaf = params.signed_by(&key, &issuer).unwrap();

    let crl = rcgen::CertificateRevocationListParams {
        this_update: rcgen::date_time_ymd(2026, 1, 1),
        next_update: rcgen::date_time_ymd(2040, 1, 1),
        crl_number: rcgen::SerialNumber::from(1u64),
        issuing_distribution_point: None,
        revoked_certs: vec![],
        key_identifier_method: rcgen::KeyIdMethod::Sha256,
    }
    .signed_by(&issuer)
    .unwrap();
    let chain = PrivateKeyChain::new(
        b"k1".to_vec(),
        PrivateKey::from_der(&key.serialize_der()).unwrap(),
        [
            Certificate::from_der(leaf.der()).unwrap(),
            Certificate::from_der(ca.der()).unwrap(),
        ],
    );
    let mut store = KeyStore::new();
    store.add_entry("firma", KeyStoreEntry::PrivateKeyChain(chain));
    let p12 = store.writer(PASSWORD).write().unwrap();
    TestPki {
        crl_der: crl.der().to_vec(),
        ca_der: ca.der().to_vec(),
        p12,
    }
}

pub fn corpus_pdf() -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/basic/hola.pdf");
    std::fs::read(p).unwrap()
}
