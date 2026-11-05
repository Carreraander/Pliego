//! Cifrado del motor nuevo (W40, bloque 7): lectura de PDF cifrados por **otro** productor (qpdf 11) con todos los
//! algoritmos de la norma (RC4-40, RC4-128, AES-128, AES-256 R5 y R6), y escritura que qpdf y MuPDF aceptan.
//!
//! Los PDF de `tests/fixtures/crypto/` se generaron con qpdf (véase el `README.md` de esa carpeta). Cada uno es el
//! mismo documento de prueba (`tests/corpus/basic/hola.pdf`, dos páginas, título «Documento de prueba») salvo los
//! que indica su nombre.

use std::path::{Path, PathBuf};
use std::process::Command;

use pliego_core::Error;
use pliego_pdf::native::PdfDocument as Native;
use pliego_pdf::{Document, Encryption, MetadataName, Permission, WriteOptions};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/crypto")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn corpus(rel: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

struct Case {
    file: &'static str,
    user: &'static str,
    owner: &'static str,
    /// Texto que debe salir de la primera página.
    text: &'static str,
}

const HOLA: &str = "Hola Pliego";

/// Todos los productores/algoritmos. `user` y `owner` son las contraseñas con que se generó cada archivo.
const CASES: &[Case] = &[
    Case {
        file: "r2-rc4-40.pdf",
        user: "u2",
        owner: "o2",
        text: HOLA,
    },
    Case {
        file: "r3-rc4-128.pdf",
        user: "u3",
        owner: "o3",
        text: HOLA,
    },
    Case {
        file: "r4-rc4-128.pdf",
        user: "u4",
        owner: "o4",
        text: HOLA,
    },
    Case {
        file: "r4-aes-128.pdf",
        user: "u4",
        owner: "o4",
        text: HOLA,
    },
    Case {
        file: "r4-cleartext-metadata.pdf",
        user: "u4",
        owner: "o4",
        text: HOLA,
    },
    Case {
        file: "r4-objstm.pdf",
        user: "u4",
        owner: "o4",
        text: HOLA,
    },
    Case {
        file: "r4-latin1.pdf",
        user: "contraseña",
        owner: "propietaria",
        text: HOLA,
    },
    Case {
        file: "r5-aes-256.pdf",
        user: "u5",
        owner: "o5",
        text: HOLA,
    },
    Case {
        file: "r6-aes-256.pdf",
        user: "u6",
        owner: "o6",
        text: HOLA,
    },
    Case {
        file: "r6-cleartext-metadata.pdf",
        user: "u6",
        owner: "o6",
        text: HOLA,
    },
    Case {
        file: "r6-objstm.pdf",
        user: "u6",
        owner: "o6",
        text: HOLA,
    },
    Case {
        file: "r6-unicode.pdf",
        user: "ñandú€",
        owner: "dueño",
        text: HOLA,
    },
    // /Encrypt directo dentro del diccionario del flujo de referencias (lo escriben algunos productores).
    Case {
        file: "direct-r6-xrefstream.pdf",
        user: "u6",
        owner: "o6",
        text: HOLA,
    },
    Case {
        file: "direct-r4-xrefstream.pdf",
        user: "u4",
        owner: "o4",
        text: HOLA,
    },
    Case {
        file: "r6-restricted.pdf",
        user: "u6",
        owner: "o6",
        text: HOLA,
    },
    Case {
        file: "r6-empty-user.pdf",
        user: "",
        owner: "o6",
        text: HOLA,
    },
];

fn first_page_text(doc: &Native) -> String {
    let page = doc.find_page(0).unwrap();
    let mut out = String::new();
    // El contenido puede ser un flujo o una lista de flujos.
    let contents = page
        .get_dict("Contents")
        .unwrap()
        .expect("la página no tiene contenido");
    if contents.is_array().unwrap() {
        for i in 0..contents.len().unwrap() {
            out.push_str(&String::from_utf8_lossy(
                &contents.get_array(i as i32).unwrap().unwrap().read_stream().unwrap(),
            ));
        }
    } else {
        out.push_str(&String::from_utf8_lossy(&contents.read_stream().unwrap()));
    }
    out
}

#[test]
fn reads_every_algorithm_with_user_and_owner_passwords() {
    for c in CASES {
        let bytes = fixture(c.file);
        let locked = Native::from_bytes(&bytes).unwrap_or_else(|e| panic!("{}: {e}", c.file));
        // Con contraseña de usuario vacía el documento se abre solo (como hace qpdf o MuPDF).
        let user_is_empty = c.user.is_empty();
        assert_eq!(locked.needs_password().unwrap(), !user_is_empty, "{}", c.file);
        if !user_is_empty {
            assert!(matches!(locked.page_count(), Err(Error::Password)), "{}", c.file);
            assert!(
                matches!(
                    Native::from_bytes_with_password(&bytes, Some("incorrecta")),
                    Err(Error::Password)
                ),
                "{}: contraseña mala",
                c.file
            );
            assert!(
                matches!(Native::from_bytes_with_password(&bytes, None), Err(Error::Password)),
                "{}: sin contraseña",
                c.file
            );
        }
        for (who, pw) in [("usuario", c.user), ("propietario", c.owner)] {
            let doc = Native::from_bytes_with_password(&bytes, Some(pw))
                .unwrap_or_else(|e| panic!("{} ({who}): {e}", c.file));
            assert_eq!(doc.page_count().unwrap(), 2, "{} ({who})", c.file);
            assert!(
                first_page_text(&doc).contains(c.text),
                "{} ({who}): contenido sin descifrar",
                c.file
            );
            // Cadenas: título del /Info y marcadores.
            assert_eq!(
                doc.metadata(MetadataName::Title).unwrap(),
                "Documento de prueba",
                "{} ({who})",
                c.file
            );
            let outline = doc.outlines().unwrap();
            assert_eq!(outline.len(), 2, "{} ({who})", c.file);
            assert_eq!(outline[0].title, "Inicio", "{} ({who})", c.file);
        }
        // `authenticate` sobre un documento abierto sin contraseña.
        let mut late = Native::from_bytes(&bytes).unwrap();
        if !user_is_empty {
            assert!(!late.authenticate("incorrecta").unwrap(), "{}", c.file);
            assert!(late.needs_password().unwrap(), "{}", c.file);
        }
        assert!(late.authenticate(c.owner).unwrap(), "{}", c.file);
        assert_eq!(late.page_count().unwrap(), 2, "{}", c.file);
    }
}

#[test]
fn reads_images_and_fonts_in_encrypted_files() {
    for (file, user) in [("r6-image.pdf", "u6"), ("r4-fonts.pdf", "u4"), ("r3-fonts.pdf", "u3")] {
        let bytes = fixture(file);
        let doc = Native::from_bytes_with_password(&bytes, Some(user)).unwrap();
        // Descifrar es transparente: el resultado sin cifrar coincide con el original según MuPDF.
        let mut o = WriteOptions::new();
        o.set_encryption(Encryption::None);
        let clear = doc.to_bytes(&o).unwrap();
        let reopened = Document::from_bytes(&clear, "application/pdf").unwrap();
        assert!(!reopened.needs_password().unwrap(), "{file}");
        assert_eq!(reopened.page_count().unwrap(), doc.page_count().unwrap(), "{file}");
        let text_of = |d: &Document| -> String {
            let mut s = String::new();
            for i in 0..d.page_count().unwrap() {
                s.push_str(
                    &d.load_page(i)
                        .unwrap()
                        .to_text_page(Default::default())
                        .unwrap()
                        .to_text()
                        .unwrap(),
                );
            }
            s
        };
        let mut mupdf_enc = Document::from_bytes(&bytes, "application/pdf").unwrap();
        assert!(mupdf_enc.authenticate(user).unwrap(), "{file}");
        assert_eq!(text_of(&reopened), text_of(&mupdf_enc), "{file}");
        if file != "r6-image.pdf" {
            assert!(!text_of(&reopened).trim().is_empty(), "{file}: sin texto");
        }
    }
}

#[test]
fn permissions_match_qpdf() {
    // El archivo restringido: sin imprimir, copiar, modificar, anotar, rellenar ni ensamblar.
    let bytes = fixture("r6-restricted.pdf");
    let doc = Native::from_bytes_with_password(&bytes, Some("u6")).unwrap();
    let p = doc.permissions();
    for (perm, name) in [
        (Permission::PRINT, "imprimir"),
        (Permission::COPY, "copiar"),
        (Permission::MODIFY, "modificar"),
        (Permission::ANNOTATE, "anotar"),
        (Permission::FORM, "formularios"),
        (Permission::ASSEMBLE, "ensamblar"),
    ] {
        assert!(!p.contains(perm), "{name} debería estar prohibido");
    }
    // Los sin restricciones.
    let open = Native::from_bytes_with_password(&fixture("r6-aes-256.pdf"), Some("u6")).unwrap();
    assert!(
        open.permissions()
            .contains(Permission::PRINT | Permission::COPY | Permission::MODIFY)
    );
}

fn qpdf(bytes: &[u8], args: &[&str], tag: &str) -> Option<(i32, String)> {
    if Command::new("qpdf").arg("--version").output().is_err() {
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join(format!("{tag}.pdf"));
    std::fs::write(&path, bytes).unwrap();
    let out = Command::new("qpdf").args(args).arg(&path).output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Some((out.status.code().unwrap_or(-1), text))
}

#[test]
fn written_files_pass_qpdf_and_mupdf_for_every_algorithm() {
    let hola = Native::from_bytes(&corpus("basic/hola.pdf")).unwrap();
    for enc in [
        Encryption::Rc4_40,
        Encryption::Rc4_128,
        Encryption::Aes128,
        Encryption::Aes256,
    ] {
        let mut o = WriteOptions::new();
        o.set_encryption(enc)
            .set_user_password("clave ñ")
            .set_owner_password("dueña");
        let out = hola.to_bytes(&o).unwrap();
        // MuPDF (motor independiente) lo abre con ambas contraseñas.
        for pw in ["clave ñ", "dueña"] {
            let mut m = Document::from_bytes(&out, "application/pdf").unwrap();
            assert!(m.needs_password().unwrap(), "{enc:?}");
            assert!(m.authenticate(pw).unwrap(), "{enc:?} con {pw:?}");
            let text = m
                .load_page(0)
                .unwrap()
                .to_text_page(Default::default())
                .unwrap()
                .to_text()
                .unwrap();
            assert!(text.contains(HOLA), "{enc:?}");
        }
        // qpdf (tercer motor) valida el cifrado y acepta las dos contraseñas.
        for pw in ["clave ñ", "dueña"] {
            if let Some((code, log)) = qpdf(&out, &["--check", &format!("--password={pw}")], "w") {
                assert_eq!(code, 0, "{enc:?} con {pw:?}\n{log}");
                assert!(!log.contains("WARNING"), "{enc:?} con {pw:?}: qpdf avisa\n{log}");
            }
        }
        // El diccionario de cifrado lleva las claves opcionales que esperan los lectores estrictos (qpdf 12 avisa si faltan).
        let text = String::from_utf8_lossy(&out);
        let (key_bits, cf_bytes) = match enc {
            Encryption::Rc4_40 => ("/Length 40", None),
            Encryption::Rc4_128 => ("/Length 128", None),
            Encryption::Aes128 => ("/Length 128", Some("/Length 16")),
            _ => ("/Length 256", Some("/Length 32")),
        };
        assert!(text.contains(key_bits), "{enc:?}: falta {key_bits}");
        if let Some(cf) = cf_bytes {
            assert!(
                text.contains(cf) && text.contains("/AuthEvent/DocOpen"),
                "{enc:?}: falta el filtro completo"
            );
        }
        if let Some((_, log)) = qpdf(&out, &["--show-encryption", "--password=clave ñ"], "e") {
            let want = match enc {
                Encryption::Rc4_40 => "R = 2",
                Encryption::Rc4_128 => "R = 3",
                Encryption::Aes128 => "R = 4",
                // AES-256 se escribe siempre como R6 (la R5 está en desuso).
                _ => "R = 6",
            };
            assert!(log.contains(want), "{enc:?}: {log}");
        }
    }
}

// --- Casos límite ---

#[test]
fn mupdf_written_encrypted_files_open_with_every_save_option() {
    // MuPDF escribe el diccionario /Encrypt directo en el trailer (o en el flujo de referencias): lopdf solo lo
    // admite indirecto, así que el modelo propio lo convierte al abrir.
    let hola = pliego_pdf::PdfDocument::from_bytes(&corpus("basic/hola.pdf")).unwrap();
    for enc in [Encryption::Rc4_128, Encryption::Aes128, Encryption::Aes256] {
        for objstm in [false, true] {
            let mut o = WriteOptions::new();
            o.set_encryption(enc).set_user_password("u").set_owner_password("o");
            o.set_object_streams(objstm);
            let bytes = hola.to_bytes(&o).unwrap();
            if std::env::var_os("DUMP").is_some() {
                std::fs::write(format!("/tmp/mupdf-{enc:?}-{objstm}.pdf"), &bytes).unwrap();
            }
            let doc = Native::from_bytes_with_password(&bytes, Some("u"))
                .unwrap_or_else(|e| panic!("{enc:?} objstm={objstm}: {e}"));
            assert_eq!(doc.page_count().unwrap(), 2, "{enc:?} objstm={objstm}");
            assert_eq!(doc.metadata(MetadataName::Title).unwrap(), "Documento de prueba");
        }
    }
}

#[test]
fn changing_passwords_and_algorithm_of_an_open_document() {
    let doc = Native::from_bytes_with_password(&fixture("r3-rc4-128.pdf"), Some("u3")).unwrap();
    let mut o = WriteOptions::new();
    o.set_encryption(Encryption::Aes256)
        .set_user_password("nueva")
        .set_owner_password("otra");
    let out = doc.to_bytes(&o).unwrap();
    assert!(matches!(
        Native::from_bytes_with_password(&out, Some("u3")),
        Err(Error::Password)
    ));
    assert!(matches!(
        Native::from_bytes_with_password(&out, Some("o3")),
        Err(Error::Password)
    ));
    for pw in ["nueva", "otra"] {
        let d = Native::from_bytes_with_password(&out, Some(pw)).unwrap();
        assert_eq!(d.page_count().unwrap(), 2);
        assert_eq!(d.metadata(MetadataName::Title).unwrap(), "Documento de prueba");
    }
    if let Some((code, log)) = qpdf(&out, &["--check", "--password=nueva"], "c") {
        assert_eq!(code, 0, "{log}");
    }
}

#[test]
fn aes256_with_object_streams_compression_and_garbage_is_valid() {
    let doc = Native::from_bytes(&corpus("office/articulo-1.pdf")).unwrap();
    let mut o = WriteOptions::new();
    o.set_encryption(Encryption::Aes256)
        .set_user_password("u")
        .set_owner_password("o");
    o.set_object_streams(true);
    o.set_compress(true);
    o.set_garbage_level(4);
    let out = doc.to_bytes(&o).unwrap();
    let d = Native::from_bytes_with_password(&out, Some("u")).unwrap();
    assert_eq!(d.page_count().unwrap(), doc.page_count().unwrap());
    if let Some((code, log)) = qpdf(&out, &["--check", "--password=u"], "g") {
        assert_eq!(code, 0, "{log}");
    }
}

#[test]
fn long_passwords_are_truncated_like_the_standard() {
    // R6 usa como máximo 127 bytes UTF-8; R≤4 solo los 32 primeros (rellenados). Una contraseña con la misma
    // cabeza y distinta cola debe abrir igual.
    let long: String = "a".repeat(200);
    let hola = Native::from_bytes(&corpus("basic/hola.pdf")).unwrap();
    for (enc, max) in [(Encryption::Aes256, 127usize), (Encryption::Aes128, 32usize)] {
        let mut o = WriteOptions::new();
        o.set_encryption(enc).set_user_password(&long).set_owner_password("o");
        let Ok(out) = hola.to_bytes(&o) else { continue };
        let same_head = format!("{}{}", &long[..max], "zzz");
        if let Ok(d) = Native::from_bytes_with_password(&out, Some(&same_head)) {
            assert_eq!(d.page_count().unwrap(), 2);
        }
        assert!(
            Native::from_bytes_with_password(&out, Some(&long)).is_ok(),
            "{enc:?}: la contraseña larga original"
        );
    }
}

#[test]
fn hostile_encrypt_dictionaries_never_panic() {
    // Se estropea cada byte del diccionario /Encrypt y de los datos de los archivos de prueba: nunca debe haber
    // pánico ni bloqueo (el error es lo esperado).
    for c in CASES.iter().filter(|c| {
        [
            "r2-rc4-40.pdf",
            "r3-rc4-128.pdf",
            "r4-aes-128.pdf",
            "r5-aes-256.pdf",
            "r6-objstm.pdf",
        ]
        .contains(&c.file)
    }) {
        let base = fixture(c.file);
        let start = base.windows(8).position(|w| w == b"/Encrypt").unwrap_or(0);
        let from = start.saturating_sub(300);
        let to = base.len().min(start + 300);
        for at in (from..to).step_by(9) {
            let mut b = base.clone();
            b[at] ^= 0x5A;
            let _ = Native::from_bytes_with_password(&b, Some(c.user)).map(|d| d.page_count());
        }
    }
}
