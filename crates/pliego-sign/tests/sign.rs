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
    // Datos añadidos que no son una actualización del documento no son un cambio permitido.
    assert!(!r.modifications_allowed);
    assert!(!r.is_valid());
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
    // La segunda firma es un cambio permitido para la primera.
    assert!(
        reports.iter().all(|r| r.modifications_allowed && r.is_valid()),
        "{reports:?}"
    );
}

#[test]
fn wrong_password_is_a_clear_error() {
    let pki = make_pki(false);
    let err = Pkcs12Signer::from_bytes(&pki.p12, "otra").err().unwrap();
    assert!(err.to_string().contains("contraseña"));
}

mod timestamp {
    use super::*;
    use pliego_sign::{LocalTsa, PadesLevel, sign_bytes_with_tsa};

    #[test]
    fn b_t_signature_carries_a_valid_timestamp() {
        let pki = make_pki(true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        // La "TSA" usa su propia clave (otra CA) para comprobar que su cadena se valida aparte.
        let tsa_pki = make_pki_for(true, true);
        let tsa = LocalTsa {
            signer: Pkcs12Signer::from_bytes(&tsa_pki.p12, PASSWORD).unwrap(),
            digest: pliego_sign::DigestAlg::Sha256,
        };
        let opts = SignOptions {
            level: PadesLevel::BT,
            ..Default::default()
        };
        let signed = sign_bytes_with_tsa(&corpus_pdf(), &signer, &opts, Some(&tsa)).unwrap();
        if let Ok(dir) = std::env::var("PLIEGO_SIGN_OUT") {
            std::fs::write(format!("{dir}/firmado-bt.pdf"), &signed).unwrap();
            std::fs::write(format!("{dir}/ca-bt.der"), &pki.ca_der).unwrap();
            std::fs::write(format!("{dir}/ca-tsa.der"), &tsa_pki.ca_der).unwrap();
        }
        let both = ValidationOptions {
            trust_anchors: vec![
                Certificate::from_der(&pki.ca_der).unwrap(),
                Certificate::from_der(&tsa_pki.ca_der).unwrap(),
            ],
        };
        let r = &validate_bytes(&signed, &both).unwrap()[0];
        assert!(r.is_valid(), "{:?}", r.problems);
        assert!(r.has_timestamp);
        let t = r.timestamp.as_ref().unwrap();
        assert!(t.valid);
        assert_eq!(t.chain, ChainStatus::Trusted);

        // Un sello de otra firma no vale: se cambia el valor de la firma sellada.
        let plain = sign_bytes(&corpus_pdf(), &signer, &SignOptions::default()).unwrap();
        assert!(!validate_bytes(&plain, &both).unwrap()[0].has_timestamp);
    }

    #[test]
    fn b_t_without_tsa_is_an_error() {
        let pki = make_pki(true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        let opts = SignOptions {
            level: PadesLevel::BT,
            ..Default::default()
        };
        assert!(sign_bytes(&corpus_pdf(), &signer, &opts).is_err());
    }
}

mod long_term {
    use super::*;
    use pliego_sign::{LocalTsa, PadesLevel, RevocationData, StaticRevocation, sign_bytes_with};

    #[test]
    fn b_lt_adds_dss_and_stays_valid() {
        let pki = make_pki(true);
        let tsa_pki = make_pki_for(true, true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        let tsa = LocalTsa {
            signer: Pkcs12Signer::from_bytes(&tsa_pki.p12, PASSWORD).unwrap(),
            digest: pliego_sign::DigestAlg::Sha256,
        };
        let rev = StaticRevocation(RevocationData {
            crls: vec![pki.crl_der.clone(), tsa_pki.crl_der.clone()],
            ocsps: vec![],
        });
        let opts = SignOptions {
            level: PadesLevel::BLt,
            ..Default::default()
        };
        let signed = sign_bytes_with(&corpus_pdf(), &signer, &opts, Some(&tsa), Some(&rev)).unwrap();
        if let Ok(dir) = std::env::var("PLIEGO_SIGN_OUT") {
            std::fs::write(format!("{dir}/firmado-blt.pdf"), &signed).unwrap();
            std::fs::write(format!("{dir}/ca-blt.der"), &pki.ca_der).unwrap();
            std::fs::write(format!("{dir}/ca-blt-tsa.der"), &tsa_pki.ca_der).unwrap();
        }
        let both = ValidationOptions {
            trust_anchors: vec![
                Certificate::from_der(&pki.ca_der).unwrap(),
                Certificate::from_der(&tsa_pki.ca_der).unwrap(),
            ],
        };
        let r = &validate_bytes(&signed, &both).unwrap()[0];
        assert!(r.has_dss && r.has_timestamp);
        assert_eq!(r.level, PadesLevel::BLt);
        assert!(!r.covers_whole_document, "el DSS va después de la firma");
        assert!(r.modifications_allowed && r.is_valid(), "{:?}", r.problems);
    }

    #[test]
    fn b_lt_without_any_revocation_data_fails() {
        let pki = make_pki(true);
        let tsa_pki = make_pki_for(true, true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        let tsa = LocalTsa {
            signer: Pkcs12Signer::from_bytes(&tsa_pki.p12, PASSWORD).unwrap(),
            digest: pliego_sign::DigestAlg::Sha256,
        };
        let rev = StaticRevocation(RevocationData::default());
        let opts = SignOptions {
            level: PadesLevel::BLt,
            ..Default::default()
        };
        let err = sign_bytes_with(&corpus_pdf(), &signer, &opts, Some(&tsa), Some(&rev)).unwrap_err();
        assert!(err.to_string().contains("revocación"));
    }

    #[test]
    fn dss_after_signing_is_allowed_but_content_edits_are_not() {
        let pki = make_pki(true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        let signed = sign_bytes(&corpus_pdf(), &signer, &SignOptions::default()).unwrap();

        // Un DSS legítimo sobre el mismo documento es un cambio permitido.
        let rev = StaticRevocation(RevocationData {
            crls: vec![pki.crl_der.clone()],
            ocsps: vec![],
        });
        let chain = pliego_sign::Signer::certificate_chain(&signer);
        let data = pliego_sign::collect_revocation(&chain, &rev).unwrap();
        let with_dss = pliego_sign::add_dss(&signed, &chain, &data).unwrap();
        let r = &validate_bytes(&with_dss, &anchors(&pki)).unwrap()[0];
        assert!(r.modifications_allowed, "{:?}", r.problems);

        // Reescribir el contenido de la página 1 (objeto 2) en una actualización incremental no lo es.
        let text = String::from_utf8_lossy(&signed).to_string();
        let prev: usize = text
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let mut forged = signed.clone();
        let obj_at = forged.len();
        forged.extend_from_slice(b"2 0 obj\n<</Length 9>>\nstream\nBT ET    \nendstream\nendobj\n");
        let xref_at = forged.len();
        forged.extend_from_slice(
            format!(
                "xref\n2 1\n{obj_at:010} 00000 n \r\ntrailer\n<</Size 14/Root 11 0 R/Info 10 0 R/Prev {prev}>>\nstartxref\n{xref_at}\n%%EOF\n"
            )
            .as_bytes(),
        );
        let r = &validate_bytes(&forged, &anchors(&pki)).unwrap()[0];
        assert!(r.integrity, "la parte firmada no cambió");
        assert!(!r.modifications_allowed);
        assert!(!r.is_valid());
        assert!(
            r.problems.iter().any(|p| p.contains("cambios no permitidos")),
            "{:?}",
            r.problems
        );
    }
}

mod http {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;
    use pliego_sign::{DigestAlg, LocalTsa, PadesLevel, TimestampAuthority};

    /// Servidor HTTP mínimo que atiende una petición RFC 3161 con una TSA local.
    fn serve_tsa_once(tsa: LocalTsa<Pkcs12Signer>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/tsa", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head_end, len) = loop {
                let n = s.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..p]).to_lowercase();
                    let len: usize = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .map(|v| v.trim().parse().unwrap())
                        .unwrap();
                    break (p + 4, len);
                }
            };
            while buf.len() < head_end + len {
                let n = s.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
            }
            let resp = tsa.timestamp(&buf[head_end..head_end + len]).unwrap();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/timestamp-reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                resp.len()
            );
            s.write_all(head.as_bytes()).unwrap();
            s.write_all(&resp).unwrap();
        });
        url
    }

    #[test]
    fn http_tsa_client_gets_a_token_over_the_wire() {
        let pki = make_pki(true);
        let tsa_pki = make_pki_for(true, true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        let url = serve_tsa_once(LocalTsa {
            signer: Pkcs12Signer::from_bytes(&tsa_pki.p12, PASSWORD).unwrap(),
            digest: DigestAlg::Sha256,
        });
        let opts = SignOptions {
            level: PadesLevel::BT,
            timestamp_url: Some(url),
            ..Default::default()
        };
        let signed = sign_bytes(&corpus_pdf(), &signer, &opts).unwrap();
        let both = ValidationOptions {
            trust_anchors: vec![
                Certificate::from_der(&pki.ca_der).unwrap(),
                Certificate::from_der(&tsa_pki.ca_der).unwrap(),
            ],
        };
        let r = &validate_bytes(&signed, &both).unwrap()[0];
        assert!(r.is_valid() && r.has_timestamp, "{:?}", r.problems);
    }

    #[test]
    fn https_tsa_is_attempted_and_failure_is_clear() {
        let pki = make_pki(true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        let opts = SignOptions {
            level: PadesLevel::BT,
            timestamp_url: Some("https://127.0.0.1:1/ts".into()),
            ..Default::default()
        };
        let err = sign_bytes(&corpus_pdf(), &signer, &opts).unwrap_err();
        assert!(err.to_string().contains("no se pudo contactar"), "{err}");
    }
}

mod visible {
    use super::*;
    use pliego_sign::VisibleSignature;

    #[test]
    fn visible_signature_has_rect_and_appearance_and_validates() {
        let pki = make_pki(true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        let opts = SignOptions {
            visible: Some(VisibleSignature {
                page: 1,
                rect: [72.0, 700.0, 220.0, 60.0],
            }),
            reason: Some("Aprobación (revisada)".into()),
            ..Default::default()
        };
        let signed = sign_bytes(&corpus_pdf(), &signer, &opts).unwrap();
        let r = &validate_bytes(&signed, &anchors(&pki)).unwrap()[0];
        assert!(r.is_valid(), "{:?}", r.problems);

        let doc = lopdf::Document::load_mem(&signed).unwrap();
        let (_, widget) = doc
            .objects
            .iter()
            .find(|(_, o)| {
                o.as_dict()
                    .is_ok_and(|d| d.get(b"FT").ok() == Some(&lopdf::Object::Name(b"Sig".to_vec())))
            })
            .unwrap();
        let d = widget.as_dict().unwrap();
        let rect: Vec<f32> = d
            .get(b"Rect")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o.as_float().unwrap())
            .collect();
        assert_eq!(rect, vec![72.0, 700.0, 292.0, 760.0]);
        let ap = d
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap()
            .as_reference()
            .unwrap();
        let stream = doc.get_object(ap).unwrap().as_stream().unwrap();
        let content = String::from_utf8_lossy(&stream.content).to_string();
        assert!(
            content.contains("Firmado digitalmente por Persona de Prueba"),
            "{content}"
        );
        assert!(content.contains("Motivo: Aprobaci"));
        // El widget cuelga de la página 2 (índice 1).
        let page2 = *doc.get_pages().get(&2).unwrap();
        assert_eq!(d.get(b"P").unwrap().as_reference().unwrap(), page2);
    }

    #[test]
    fn tiny_or_missing_page_is_rejected() {
        let pki = make_pki(true);
        let signer = Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap();
        let small = SignOptions {
            visible: Some(VisibleSignature {
                page: 0,
                rect: [0.0, 0.0, 5.0, 5.0],
            }),
            ..Default::default()
        };
        assert!(sign_bytes(&corpus_pdf(), &signer, &small).is_err());
        let no_page = SignOptions {
            visible: Some(VisibleSignature {
                page: 9,
                rect: [0.0, 0.0, 100.0, 40.0],
            }),
            ..Default::default()
        };
        assert!(sign_bytes(&corpus_pdf(), &signer, &no_page).is_err());
    }
}

mod fields_and_certification {
    use super::*;

    fn signer(pki: &TestPki) -> Pkcs12Signer {
        Pkcs12Signer::from_bytes(&pki.p12, PASSWORD).unwrap()
    }

    #[test]
    fn signs_an_existing_empty_field_in_place() {
        let pki = make_pki(true);
        let opts = SignOptions {
            field_name: Some("Firma1".into()),
            ..Default::default()
        };
        let signed = sign_bytes(&pdf_with_empty_field(), &signer(&pki), &opts).unwrap();
        let r = &validate_bytes(&signed, &anchors(&pki)).unwrap()[0];
        assert!(r.is_valid(), "{:?}", r.problems);
        assert_eq!(r.field.as_deref(), Some("Firma1"));

        let doc = lopdf::Document::load_mem(&signed).unwrap();
        let sig_fields = doc
            .objects
            .values()
            .filter(|o| {
                o.as_dict()
                    .is_ok_and(|d| d.get(b"FT").ok() == Some(&lopdf::Object::Name(b"Sig".to_vec())))
            })
            .count();
        assert_eq!(sig_fields, 1, "no debe crearse un campo nuevo");
        // El campo tenía tamaño: ahora muestra la apariencia.
        let field = doc
            .objects
            .values()
            .find(|o| o.as_dict().is_ok_and(|d| d.has(b"FT")))
            .unwrap();
        assert!(field.as_dict().unwrap().has(b"AP"));
    }

    #[test]
    fn an_already_signed_field_cannot_be_signed_again() {
        let pki = make_pki(true);
        let opts = SignOptions {
            field_name: Some("Firma1".into()),
            ..Default::default()
        };
        let once = sign_bytes(&pdf_with_empty_field(), &signer(&pki), &opts).unwrap();
        let err = sign_bytes(&once, &signer(&pki), &opts).unwrap_err();
        assert!(err.to_string().contains("ya está firmado"), "{err}");
    }

    #[test]
    fn certification_levels_control_later_changes() {
        let pki = make_pki(true);
        let s = signer(&pki);
        for (level, later_signature_ok) in [(2u8, true), (1u8, false)] {
            let opts = SignOptions {
                certify: Some(level),
                ..Default::default()
            };
            let certified = sign_bytes(&corpus_pdf(), &s, &opts).unwrap();
            if let Ok(dir) = std::env::var("PLIEGO_SIGN_OUT") {
                std::fs::write(format!("{dir}/certificado-p{level}.pdf"), &certified).unwrap();
                std::fs::write(format!("{dir}/ca-cert.der"), &pki.ca_der).unwrap();
            }
            let r = &validate_bytes(&certified, &anchors(&pki)).unwrap()[0];
            assert_eq!(r.certification, Some(level));
            assert!(r.is_valid(), "{:?}", r.problems);

            let again = sign_bytes(&certified, &s, &SignOptions::default()).unwrap();
            let reports = validate_bytes(&again, &anchors(&pki)).unwrap();
            let first = reports.iter().find(|r| r.certification.is_some()).unwrap();
            assert!(first.integrity);
            assert_eq!(
                first.modifications_allowed, later_signature_ok,
                "P={level}: {:?}",
                first.problems
            );
            // Un sello de mantenimiento (DSS) sí se admite incluso con P=1.
            let chain = pliego_sign::Signer::certificate_chain(&s);
            let rev = pliego_sign::StaticRevocation(pliego_sign::RevocationData {
                crls: vec![pki.crl_der.clone()],
                ocsps: vec![],
            });
            let data = pliego_sign::collect_revocation(&chain, &rev).unwrap();
            let with_dss = pliego_sign::add_dss(&certified, &chain, &data).unwrap();
            let r = &validate_bytes(&with_dss, &anchors(&pki)).unwrap()[0];
            assert!(r.modifications_allowed, "P={level}: {:?}", r.problems);
        }
    }

    #[test]
    fn only_the_first_signature_can_certify_and_levels_are_checked() {
        let pki = make_pki(true);
        let s = signer(&pki);
        let once = sign_bytes(&corpus_pdf(), &s, &SignOptions::default()).unwrap();
        let opts = SignOptions {
            certify: Some(2),
            ..Default::default()
        };
        assert!(sign_bytes(&once, &s, &opts).is_err());
        let bad = SignOptions {
            certify: Some(7),
            ..Default::default()
        };
        assert!(sign_bytes(&corpus_pdf(), &s, &bad).is_err());
    }
}
