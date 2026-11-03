use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
use pliego_sign::{Pkcs12Signer, SignOptions, ValidationOptions, validate_bytes};
use pliego_signflow::{AuditEvent, SignerSpec, Signers, audit_log, next_signer, prepare, read_manifest, sign_next};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose};

fn signer_for(cn: &str) -> Pkcs12Signer {
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.distinguished_name.push(DnType::CommonName, "CA de pruebas");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = Issuer::new(ca_params, ca_key);
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.distinguished_name.push(DnType::CommonName, cn);
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::ContentCommitment];
    let leaf = params.signed_by(&key, &issuer).unwrap();
    let chain = PrivateKeyChain::new(
        b"k".to_vec(),
        PrivateKey::from_der(&key.serialize_der()).unwrap(),
        [
            Certificate::from_der(leaf.der()).unwrap(),
            Certificate::from_der(ca.der()).unwrap(),
        ],
    );
    let mut store = KeyStore::new();
    store.add_entry("f", KeyStoreEntry::PrivateKeyChain(chain));
    Pkcs12Signer::from_bytes(&store.writer("x").write().unwrap(), "x").unwrap()
}

fn corpus() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/basic/hola.pdf"
    ))
    .unwrap()
}

fn three() -> Signers {
    Signers::new(vec![
        SignerSpec::new("Ana"),
        SignerSpec::new("Beto"),
        SignerSpec::new("Carla"),
    ])
}

#[test]
fn three_signers_in_order() {
    let pkg = prepare(&corpus(), &three()).unwrap();
    assert_eq!(read_manifest(&pkg.pdf).unwrap(), pkg.manifest);
    let mut doc = pkg.pdf;
    for (i, name) in ["Ana", "Beto", "Carla"].iter().enumerate() {
        let p = next_signer(&doc).unwrap().expect("pendiente");
        assert_eq!((p.index, p.spec.name.as_str()), (i, *name));
        let (out, after) = sign_next(&doc, &signer_for(name), SignOptions::default()).unwrap();
        assert_eq!(after.is_some(), i < 2);
        doc = out;
    }
    assert!(next_signer(&doc).unwrap().is_none());
    let reports = validate_bytes(&doc, &ValidationOptions::default()).unwrap();
    assert_eq!(reports.len(), 3);
    assert!(reports.iter().all(|r| r.integrity));
    let log = audit_log(&doc).unwrap();
    assert_eq!(log.len(), 4);
    for (i, e) in log[1..].iter().enumerate() {
        let AuditEvent::Signed {
            index,
            in_order,
            integrity,
            certificate,
            ..
        } = &e.event
        else {
            panic!()
        };
        assert_eq!(*index, Some(i));
        assert!(*in_order && *integrity);
        assert_eq!(certificate, ["Ana", "Beto", "Carla"][i]);
    }
    assert!(sign_next(&doc, &signer_for("Ana"), SignOptions::default()).is_err());
}

#[test]
fn rejects_wrong_certificate_and_tampering() {
    let mut s = three();
    s.0[0].certificate_contains = Some("Ana".into());
    let pkg = prepare(&corpus(), &s).unwrap();
    assert!(sign_next(&pkg.pdf, &signer_for("Otro"), SignOptions::default()).is_err());
    let (mut signed, _) = sign_next(&pkg.pdf, &signer_for("Ana"), SignOptions::default()).unwrap();
    // Altera un byte cubierto por la firma.
    signed[20] ^= 1;
    assert!(next_signer(&signed).is_err());
}

#[test]
fn rejects_bad_input() {
    assert!(prepare(&corpus(), &Signers::default()).is_err());
    let pkg = prepare(&corpus(), &three()).unwrap();
    assert!(prepare(&pkg.pdf, &three()).is_err());
    assert!(next_signer(&corpus()).is_err());
    let (signed, _) = sign_next(&pkg.pdf, &signer_for("Ana"), SignOptions::default()).unwrap();
    assert!(prepare(&signed, &three()).is_err());
}
