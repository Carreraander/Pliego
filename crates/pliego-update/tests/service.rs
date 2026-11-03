//! El servicio de actualizaciones de extremo a extremo contra un servidor local.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use pliego_core::JobContext;
use pliego_update::sign::{ReleaseKey, seal};
use pliego_update::{
    Artifact, ArtifactKind, InstallKind, Manifest, Phase, Release, Stager, UpdateConfig, UpdateMode, UpdateService,
    Version,
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
    thread::spawn(move || {
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

fn publish(root: &Path, key: &ReleaseKey, version: &str, critical: bool) {
    std::fs::create_dir_all(root.join("packages")).unwrap();
    std::fs::create_dir_all(root.join("v1/stable")).unwrap();
    let package = vec![7u8; 50_000];
    let name = format!("pliego-{version}.paq");
    std::fs::write(root.join("packages").join(&name), &package).unwrap();
    let sha: String = Sha256::digest(&package).iter().map(|b| format!("{b:02x}")).collect();
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
            critical,
            rollout_percent: 100,
            min_windows_build: None,
            artifacts: vec![Artifact {
                kind: ArtifactKind::Package,
                arch: std::env::consts::ARCH.into(),
                url: format!("packages/{name}"),
                size: package.len() as u64,
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

#[derive(Default)]
struct Recorder(RefCell<Vec<Version>>);

impl Stager for &Recorder {
    fn stage(&self, package: &Path, version: &Version) -> Result<(), String> {
        assert!(package.is_file());
        self.0.borrow_mut().push(version.clone());
        Ok(())
    }
}

fn open<'a>(
    s: &Server,
    key: &ReleaseKey,
    kind: InstallKind,
    mode: UpdateMode,
    rec: &'a Recorder,
    tmp: &Path,
) -> UpdateService<&'a Recorder> {
    let mut config = UpdateConfig::new(&s.url, "stable", Version::new(0, 1, 0), vec![key.public()]);
    config.mode = mode;
    UpdateService::open(config, kind, tmp.join("state.json"), tmp.join("updates"), rec, None).unwrap()
}

#[test]
fn automatic_mode_downloads_stages_and_survives_restart() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", false);
    let (rec, tmp) = (Recorder::default(), tempfile::tempdir().unwrap());
    let mut svc = open(&s, &key, InstallKind::PerUser, UpdateMode::Automatic, &rec, tmp.path());
    assert!(matches!(
        svc.run_cycle(false, &JobContext::silent()),
        Phase::Ready { .. }
    ));
    assert_eq!(*rec.0.borrow(), vec![Version::new(0, 1, 1)]);
    // Al reabrir la app, la versión preparada sigue lista sin pedir nada al servidor.
    let before = s.hits.load(Ordering::SeqCst);
    let svc = open(&s, &key, InstallKind::PerUser, UpdateMode::Automatic, &rec, tmp.path());
    assert!(matches!(svc.status().phase, Phase::Ready { .. }));
    assert_eq!(s.hits.load(Ordering::SeqCst), before);
}

#[test]
fn notify_only_and_metered_wait_for_the_user_but_critical_goes_ahead() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", false);
    let (rec, tmp) = (Recorder::default(), tempfile::tempdir().unwrap());
    let mut svc = open(&s, &key, InstallKind::PerUser, UpdateMode::NotifyOnly, &rec, tmp.path());
    assert!(matches!(
        svc.run_cycle(false, &JobContext::silent()),
        Phase::Available { can_download: true, .. }
    ));
    assert!(rec.0.borrow().is_empty());
    assert!(matches!(svc.download(&JobContext::silent()), Phase::Ready { .. }));

    let tmp2 = tempfile::tempdir().unwrap();
    let mut metered = open(&s, &key, InstallKind::PerUser, UpdateMode::Automatic, &rec, tmp2.path());
    assert!(matches!(
        metered.run_cycle(true, &JobContext::silent()),
        Phase::Available { .. }
    ));

    publish(&s.root, &key, "0.1.1", true);
    let tmp3 = tempfile::tempdir().unwrap();
    let mut crit = open(
        &s,
        &key,
        InstallKind::PerUser,
        UpdateMode::NotifyOnly,
        &rec,
        tmp3.path(),
    );
    assert!(matches!(
        crit.run_cycle(true, &JobContext::silent()),
        Phase::Ready { mandatory: true, .. }
    ));
}

#[test]
fn portable_only_warns_and_msi_never_asks() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", false);
    let rec = Recorder::default();
    let t1 = tempfile::tempdir().unwrap();
    let mut portable = open(&s, &key, InstallKind::Portable, UpdateMode::Automatic, &rec, t1.path());
    assert!(matches!(
        portable.run_cycle(false, &JobContext::silent()),
        Phase::Available {
            can_download: false,
            ..
        }
    ));
    assert!(rec.0.borrow().is_empty());
    let hits = s.hits.load(Ordering::SeqCst);
    let t2 = tempfile::tempdir().unwrap();
    let mut msi = open(&s, &key, InstallKind::Msi, UpdateMode::Automatic, &rec, t2.path());
    assert_eq!(msi.run_cycle(false, &JobContext::silent()), &Phase::Disabled);
    assert_eq!(s.hits.load(Ordering::SeqCst), hits);
    assert!(msi.next_check_at().is_none());
}

#[test]
fn unconfigured_or_disabled_makes_no_requests() {
    let s = server();
    let key = ReleaseKey::from_seed(&[1; 32]);
    publish(&s.root, &key, "0.1.1", false);
    let (rec, tmp) = (Recorder::default(), tempfile::tempdir().unwrap());
    // Sin claves compiladas.
    let no_keys = UpdateConfig::new(&s.url, "stable", Version::new(0, 1, 0), vec![]);
    let mut svc = UpdateService::open(
        no_keys,
        InstallKind::PerUser,
        tmp.path().join("a.json"),
        tmp.path().join("u"),
        &rec,
        None,
    )
    .unwrap();
    assert_eq!(svc.run_cycle(false, &JobContext::silent()), &Phase::NotConfigured);
    // Sin URL.
    let mut no_url = UpdateConfig::new(&s.url, "stable", Version::new(0, 1, 0), vec![key.public()]);
    no_url.base_url = None;
    let mut svc2 = UpdateService::open(
        no_url,
        InstallKind::PerUser,
        tmp.path().join("b.json"),
        tmp.path().join("u"),
        &rec,
        None,
    )
    .unwrap();
    assert_eq!(svc2.run_cycle(false, &JobContext::silent()), &Phase::NotConfigured);
    // Modo desactivado.
    let mut svc3 = open(&s, &key, InstallKind::PerUser, UpdateMode::Disabled, &rec, tmp.path());
    assert_eq!(svc3.run_cycle(false, &JobContext::silent()), &Phase::Disabled);
    assert_eq!(s.hits.load(Ordering::SeqCst), 0);
}

#[test]
fn network_error_is_plain_and_retries_in_an_hour() {
    let key = ReleaseKey::from_seed(&[1; 32]);
    let (rec, tmp) = (Recorder::default(), tempfile::tempdir().unwrap());
    let config = UpdateConfig::new(
        "http://127.0.0.1:1",
        "stable",
        Version::new(0, 1, 0),
        vec![key.public()],
    );
    let mut svc = UpdateService::open(
        config,
        InstallKind::PerUser,
        tmp.path().join("s.json"),
        tmp.path().join("u"),
        &rec,
        None,
    )
    .unwrap();
    let Phase::Failed { message, .. } = svc.run_cycle(false, &JobContext::silent()).clone() else {
        panic!("debería fallar");
    };
    assert!(message.starts_with("No se pudo comprobar"));
    let due = svc.next_check_at().unwrap();
    assert!(due > time::OffsetDateTime::now_utc() + time::Duration::minutes(59));
}
