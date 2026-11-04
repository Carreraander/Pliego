//! El hilo de la app (`Runner`) contra un servidor local: comprobar, descargar, preparar y avisar de cada cambio.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pliego_install::Layout;
use pliego_update::sign::{ReleaseKey, seal};
use pliego_update::{
    Artifact, ArtifactKind, InstallKind, LayoutStager, Manifest, Phase, Release, Runner, Snapshot, UpdateConfig,
    UpdateMode, UpdateService, Version,
};
use sha2::{Digest, Sha256};

struct Server {
    url: String,
    hits: Arc<AtomicUsize>,
    root: PathBuf,
    _dir: tempfile::TempDir,
}

fn server() -> Server {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let srv = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", srv.server_addr().to_ip().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let (r, h) = (root.clone(), hits.clone());
    std::thread::spawn(move || {
        for req in srv.incoming_requests() {
            h.fetch_add(1, Ordering::SeqCst);
            let file = r.join(req.url().trim_start_matches('/'));
            let _ = match std::fs::read(&file) {
                Ok(b) => req.respond(tiny_http::Response::from_data(b)),
                Err(_) => req.respond(tiny_http::Response::from_string("no").with_status_code(404)),
            };
        }
    });
    Server {
        url,
        hits,
        root,
        _dir: dir,
    }
}

/// Publica `version` con un `.paq` real. `tamper`: cambia un byte del paquete después de firmar su hash.
fn publish(root: &Path, key: &ReleaseKey, version: &str, tamper: bool) {
    let work = tempfile::tempdir().unwrap();
    let src = work.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("pliego-app.exe"), format!("app {version}")).unwrap();
    let paq = work.path().join("p.paq");
    pliego_install::paq::pack(&src, version, &paq).unwrap();
    let mut bytes = std::fs::read(&paq).unwrap();
    let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let size = bytes.len() as u64;
    if tamper {
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xff;
    }
    std::fs::create_dir_all(root.join("packages")).unwrap();
    std::fs::create_dir_all(root.join("v1/stable")).unwrap();
    std::fs::write(root.join(format!("packages/pliego-{version}.paq")), &bytes).unwrap();
    let manifest = Manifest {
        schema: 1,
        product: "pliego".into(),
        channel: "stable".into(),
        serial: 1,
        issued_at: "2026-01-01T00:00:00Z".into(),
        expires_at: "2099-01-01T00:00:00Z".into(),
        latest: Release {
            version: version.parse().unwrap(),
            released_at: "2026-01-01T00:00:00Z".into(),
            notes: BTreeMap::from([("es".to_owned(), "- Mejoras".to_owned())]),
            notes_url: None,
            critical: false,
            rollout_percent: 100,
            min_windows_build: None,
            artifacts: vec![Artifact {
                kind: ArtifactKind::Package,
                arch: std::env::consts::ARCH.into(),
                url: format!("packages/pliego-{version}.paq"),
                size,
                sha256: sha,
            }],
        },
        min_supported_version: None,
        blocked_versions: vec![],
    };
    std::fs::write(
        root.join("v1/stable/manifest.json"),
        serde_json::to_vec(&seal(&manifest, &[key])).unwrap(),
    )
    .unwrap();
}

struct Fixture {
    runner: Runner,
    events: Arc<Mutex<Vec<Snapshot>>>,
    install: PathBuf,
    _tmp: tempfile::TempDir,
}

fn start(url: Option<&str>, keys: Vec<pliego_update::PublicKey>, mode: UpdateMode) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let install = tmp.path().join("Pliego");
    std::fs::create_dir_all(&install).unwrap();
    Layout::new(&install).write_atomic("current", "0.1.0").unwrap();
    let mut config = UpdateConfig::new(
        url.unwrap_or("http://127.0.0.1:1"),
        "stable",
        Version::new(0, 1, 0),
        keys,
    );
    if url.is_none() {
        config.base_url = None;
    }
    config.mode = mode;
    let downloads = tmp.path().join("updates");
    let service = UpdateService::open(
        config,
        InstallKind::PerUser,
        tmp.path().join("update-state.json"),
        downloads.clone(),
        LayoutStager::new(InstallKind::PerUser, &install, &downloads),
        None,
    )
    .unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let runner = Runner::start(service, true, || false, move |s| sink.lock().unwrap().push(s.clone()));
    Fixture {
        runner,
        events,
        install,
        _tmp: tmp,
    }
}

fn wait_for(f: &Fixture, what: &str, pred: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        let s = f.runner.snapshot();
        if !s.busy && pred(&s) {
            return s;
        }
        assert!(Instant::now() < end, "no se llegó a «{what}»: {:?}", s.status.phase);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn automatic_mode_downloads_stages_and_reports_progress() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", false);
    let f = start(Some(&s.url), vec![key.public()], UpdateMode::Automatic);
    f.runner.check_now();
    let snap = wait_for(&f, "Ready", |s| matches!(s.status.phase, Phase::Ready { .. }));
    let Phase::Ready { version, notes, .. } = snap.status.phase else {
        unreachable!()
    };
    assert_eq!(version, Version::new(0, 1, 1));
    assert_eq!(notes.as_deref(), Some("- Mejoras"));
    let layout = Layout::new(&f.install);
    assert_eq!(layout.pending().unwrap().as_deref(), Some("0.1.1"));
    assert!(f.install.join("app-0.1.1/pliego-app.exe").is_file());
    let events = f.events.lock().unwrap();
    assert!(events.iter().any(|e| e.busy), "avisó de que estaba trabajando");
    assert!(
        events
            .iter()
            .any(|e| matches!(e.status.phase, Phase::Downloading { total, .. } if total > 0))
    );
}

#[test]
fn tampered_package_is_rejected_and_nothing_is_staged() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", true);
    let f = start(Some(&s.url), vec![key.public()], UpdateMode::Automatic);
    f.runner.check_now();
    let snap = wait_for(&f, "Failed", |s| matches!(s.status.phase, Phase::Failed { .. }));
    let Phase::Failed { message, .. } = snap.status.phase else {
        unreachable!()
    };
    assert!(message.contains("No se pudo comprobar"), "{message}");
    assert_eq!(Layout::new(&f.install).pending().unwrap(), None);
    assert!(!f.install.join("app-0.1.1").exists());
}

#[test]
fn notify_only_waits_for_the_download_button() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", false);
    let f = start(Some(&s.url), vec![key.public()], UpdateMode::NotifyOnly);
    f.runner.check_now();
    wait_for(&f, "Available", |s| {
        matches!(s.status.phase, Phase::Available { can_download: true, .. })
    });
    assert_eq!(Layout::new(&f.install).pending().unwrap(), None);
    f.runner.download();
    wait_for(&f, "Ready", |s| matches!(s.status.phase, Phase::Ready { .. }));
    assert_eq!(Layout::new(&f.install).pending().unwrap().as_deref(), Some("0.1.1"));
}

#[test]
fn without_url_or_keys_no_request_is_made() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", false);
    // Con URL pero sin claves compiladas.
    let f = start(Some(&s.url), vec![], UpdateMode::Automatic);
    f.runner.check_now();
    f.runner.download();
    std::thread::sleep(Duration::from_millis(300));
    assert!(matches!(f.runner.snapshot().status.phase, Phase::NotConfigured));
    // Con claves pero sin URL.
    let g = start(None, vec![key.public()], UpdateMode::Automatic);
    g.runner.check_now();
    std::thread::sleep(Duration::from_millis(300));
    assert!(matches!(g.runner.snapshot().status.phase, Phase::NotConfigured));
    assert_eq!(s.hits.load(Ordering::SeqCst), 0, "ni una petición");
}

#[test]
fn disabled_mode_makes_no_request_and_settings_can_reenable() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", false);
    let f = start(Some(&s.url), vec![key.public()], UpdateMode::Disabled);
    f.runner.check_now();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(s.hits.load(Ordering::SeqCst), 0);
    let mut config = UpdateConfig::new(&s.url, "stable", Version::new(0, 1, 0), vec![key.public()]);
    config.mode = UpdateMode::Automatic;
    f.runner.apply_settings(config, true);
    f.runner.check_now();
    wait_for(&f, "Ready", |s| matches!(s.status.phase, Phase::Ready { .. }));
}
