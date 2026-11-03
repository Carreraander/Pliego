//! Prueba de extremo a extremo: publicar un manifiesto firmado, servirlo por HTTP en local, comprobar y descargar.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::thread;

use pliego_core::JobContext;
use pliego_update::sign::{ReleaseKey, seal};
use pliego_update::{
    Artifact, ArtifactKind, CheckOutcome, Manifest, Release, UpdateClient, UpdateConfig, UpdateError, UpdateState,
    Version,
};
use sha2::{Digest, Sha256};

struct Server {
    url: String,
    _root: tempfile::TempDir,
    root: PathBuf,
}

/// Servidor estático en un puerto libre de 127.0.0.1.
fn server() -> Server {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_path_buf();
    let srv = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", srv.server_addr().to_ip().unwrap());
    let serve_root = path.clone();
    thread::spawn(move || {
        for req in srv.incoming_requests() {
            let file = serve_root.join(req.url().trim_start_matches('/'));
            let _ = match std::fs::read(&file) {
                Ok(bytes) => req.respond(tiny_http::Response::from_data(bytes)),
                Err(_) => req.respond(tiny_http::Response::from_string("no").with_status_code(404)),
            };
        }
    });
    Server {
        url,
        _root: root,
        root: path,
    }
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn publish(root: &Path, key: &ReleaseKey, version: &str, serial: u64, package: &[u8], expires: &str) {
    std::fs::create_dir_all(root.join("packages")).unwrap();
    std::fs::create_dir_all(root.join("v1/stable")).unwrap();
    let name = format!("pliego-{version}-x86_64.paq");
    std::fs::write(root.join("packages").join(&name), package).unwrap();
    let manifest = Manifest {
        schema: 1,
        product: "pliego".into(),
        channel: "stable".into(),
        serial,
        issued_at: "2026-01-01T00:00:00Z".into(),
        expires_at: expires.into(),
        latest: Release {
            version: Version::parse(version).unwrap(),
            released_at: "2026-01-01T00:00:00Z".into(),
            notes: BTreeMap::from([("es".to_owned(), "Arreglos".to_owned())]),
            notes_url: None,
            critical: false,
            rollout_percent: 100,
            min_windows_build: None,
            artifacts: vec![Artifact {
                kind: ArtifactKind::Package,
                arch: std::env::consts::ARCH.into(),
                url: format!("packages/{name}"),
                size: package.len() as u64,
                sha256: sha(package),
            }],
        },
        min_supported_version: None,
        blocked_versions: vec![],
    };
    let env = seal(&manifest, &[key]);
    std::fs::write(root.join("v1/stable/manifest.json"), serde_json::to_vec(&env).unwrap()).unwrap();
}

const FAR: &str = "2099-01-01T00:00:00Z";

#[test]
fn check_and_download_verified_update() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    let package = vec![42u8; 300_000];
    publish(&s.root, &key, "0.2.0", 1, &package, FAR);

    let client = UpdateClient::new(UpdateConfig::new(
        &s.url,
        "stable",
        Version::new(0, 1, 0),
        vec![key.public()],
    ))
    .unwrap();
    let mut state = UpdateState::new([9; 16]);
    let CheckOutcome::Available(v) = client.check(&mut state).unwrap() else {
        panic!("debería haber actualización");
    };
    assert_eq!(v.release.version, Version::new(0, 2, 0));
    assert_eq!(state.last_serial["stable"], 1);

    let dir = tempfile::tempdir().unwrap();
    let path = client.download(&v.package, dir.path(), &JobContext::silent()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), package);
    // Segunda descarga: reutiliza el archivo ya verificado.
    assert_eq!(
        client.download(&v.package, dir.path(), &JobContext::silent()).unwrap(),
        path
    );

    // Ya en 0.2.0: al día.
    let client2 = UpdateClient::new(UpdateConfig::new(
        &s.url,
        "stable",
        Version::new(0, 2, 0),
        vec![key.public()],
    ))
    .unwrap();
    assert_eq!(client2.check(&mut state).unwrap(), CheckOutcome::UpToDate);
}

#[test]
fn untrusted_key_is_rejected() {
    let s = server();
    let attacker = ReleaseKey::from_seed(&[66; 32]);
    let real = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &attacker, "9.9.9", 1, b"malware", FAR);
    let client = UpdateClient::new(UpdateConfig::new(
        &s.url,
        "stable",
        Version::new(0, 1, 0),
        vec![real.public()],
    ))
    .unwrap();
    assert!(matches!(
        client.check(&mut UpdateState::new([0; 16])),
        Err(UpdateError::BadSignature)
    ));
}

#[test]
fn tampered_package_is_deleted() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.2.0", 1, b"original", FAR);
    // Se cambia el paquete en el servidor después de firmar (mismo tamaño).
    std::fs::write(s.root.join("packages/pliego-0.2.0-x86_64.paq"), b"ORIGINAL").unwrap();
    let client = UpdateClient::new(UpdateConfig::new(
        &s.url,
        "stable",
        Version::new(0, 1, 0),
        vec![key.public()],
    ))
    .unwrap();
    let CheckOutcome::Available(v) = client.check(&mut UpdateState::new([0; 16])).unwrap() else {
        panic!()
    };
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        client.download(&v.package, dir.path(), &JobContext::silent()),
        Err(UpdateError::DigestMismatch)
    ));
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "no queda nada a medias"
    );
}

#[test]
fn replayed_and_expired_manifests_are_rejected() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    let client = UpdateClient::new(UpdateConfig::new(
        &s.url,
        "stable",
        Version::new(0, 1, 0),
        vec![key.public()],
    ))
    .unwrap();
    let mut state = UpdateState::new([0; 16]);
    publish(&s.root, &key, "0.3.0", 7, b"x", FAR);
    client.check(&mut state).unwrap();
    // Un atacante vuelve a servir un manifiesto antiguo, correctamente firmado, con serial menor.
    publish(&s.root, &key, "0.2.0", 6, b"x", FAR);
    assert!(matches!(client.check(&mut state), Err(UpdateError::Replayed)));
    publish(&s.root, &key, "0.3.0", 8, b"x", "2020-01-01T00:00:00Z");
    assert!(matches!(client.check(&mut state), Err(UpdateError::Expired)));
}

#[test]
fn plain_http_to_remote_hosts_is_refused() {
    let cfg = UpdateConfig::new(
        "http://actualizaciones.ejemplo.es",
        "stable",
        Version::new(0, 1, 0),
        vec![],
    );
    assert!(matches!(UpdateClient::new(cfg), Err(UpdateError::InsecureUrl(_))));
}
