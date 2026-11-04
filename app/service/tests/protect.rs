//! Pruebas de `pliego_app_service::protect`. Van en su propio ejecutable: los tests del servicio lanzan `pliego-worker`
//! y, en Windows, el proceso hijo hereda los archivos que otro test tiene abiertos en ese instante (error 32 al renombrar).

use std::path::{Path, PathBuf};

use pliego_app_service::protect::*;
use pliego_core::{Error, JobContext};
use pliego_protect::SanitizeOptions;
use pliego_redact::RedactOptions;

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

/// Copia el PDF a una carpeta temporal para que las copias de salida no ensucien el repositorio.
fn temp_copy(rel: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(Path::new(rel).file_name().unwrap());
    std::fs::copy(corpus(rel), &path).unwrap();
    (dir, path)
}

#[test]
fn copy_path_evita_colisiones() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("contrato.pdf");
    assert_eq!(
        copy_path(&input, "protegido"),
        dir.path().join("contrato (protegido).pdf")
    );
    std::fs::write(dir.path().join("contrato (protegido).pdf"), b"x").unwrap();
    assert_eq!(
        copy_path(&input, "protegido"),
        dir.path().join("contrato (protegido 2).pdf")
    );
}

#[test]
fn protect_cifra_una_copia_y_respeta_el_original() {
    let (_d, path) = temp_copy("basic/hola.pdf");
    let before = std::fs::read(&path).unwrap();
    let req: ProtectRequest = serde_json::from_value(serde_json::json!({
        "path": path, "userPassword": "abre", "ownerPassword": "manda",
        "permissions": { "print": true, "print_high_quality": true, "copy": false, "modify": false,
            "annotate": true, "fill_forms": true, "accessibility": true, "assemble": true },
    }))
    .unwrap();
    let rep = protect(&req, &JobContext::silent()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before, "el original no cambia");
    let out = &rep.outputs[0];
    assert!(out.file_name().unwrap().to_string_lossy().contains("(protegido)"));
    assert!(pliego_protect::permissions_of(out, None).is_err(), "pide contraseña");
    let perms = pliego_protect::permissions_of(out, Some("abre")).unwrap();
    assert!(!perms.copy && !perms.modify && perms.print);
}

#[test]
fn protect_sin_contrasenas_falla() {
    let (_d, path) = temp_copy("basic/hola.pdf");
    let req: ProtectRequest = serde_json::from_value(serde_json::json!({ "path": path })).unwrap();
    assert!(matches!(
        protect(&req, &JobContext::silent()),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn unprotect_quita_la_proteccion() {
    let (_d, path) = temp_copy("basic/hola.pdf");
    let req: ProtectRequest =
        serde_json::from_value(serde_json::json!({ "path": path, "userPassword": "abre" })).unwrap();
    let enc = protect(&req, &JobContext::silent()).unwrap().outputs.remove(0);
    let rep = unprotect(&enc, "abre", &JobContext::silent()).unwrap();
    assert!(
        pliego_protect::permissions_of(&rep.outputs[0], None).is_ok(),
        "ya no pide contraseña"
    );
    assert!(unprotect(&enc, "", &JobContext::silent()).is_err());
}

#[test]
fn sanitize_deja_informe() {
    let (_d, path) = temp_copy("protect/sucio.pdf");
    let req = SanitizeRequest {
        path,
        password: None,
        options: SanitizeOptions::default(),
    };
    let rep = sanitize(&req, &JobContext::silent()).unwrap();
    assert!(rep.summary.starts_with("Eliminado:"), "{}", rep.summary);
    assert!(rep.outputs[0].exists());
}

#[test]
fn busca_y_redacta_datos_personales() {
    let (_d, path) = temp_copy("redact/datos.pdf");
    let found = find_matches(&path, None, &["dni".into(), "iban".into()]).unwrap();
    assert!(!found.is_empty());
    assert!(found.iter().all(|m| !m.areas.is_empty()));
    assert!(find_matches(&path, None, &["nada".into()]).is_err());
    assert!(find_matches(&path, None, &[]).unwrap().is_empty());

    let req = RedactRequest {
        path: path.clone(),
        password: None,
        areas: vec![],
        patterns: vec!["dni".into()],
        options: RedactOptions::default(),
    };
    let rep = redact(&req, &JobContext::silent()).unwrap();
    let out = &rep.outputs[0];
    assert!(out.file_name().unwrap().to_string_lossy().contains("(redactado)"));
    assert!(
        find_matches(out, None, &["dni".into()]).unwrap().is_empty(),
        "el DNI ya no se puede extraer"
    );
}
