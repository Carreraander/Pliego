//! El registro de diagnóstico recoge los trabajos que fallan (y no los cancelados ni los correctos).
//! Va en su propio ejecutable porque el registro es global del proceso.

use std::sync::mpsc;
use std::time::Duration;

use pliego_appdata::jobs::{JobEvent, JobRegistry};
use pliego_core::{Error, Report};

#[test]
fn failed_jobs_reach_the_log_but_not_the_cancelled_or_the_successful() {
    let dir = tempfile::tempdir().unwrap();
    let logs = pliego_core::log::logs_dir(dir.path());
    pliego_core::log::init(&logs).unwrap();
    pliego_core::log::info("arranque", "Pliego de prueba");

    let (tx, rx) = mpsc::channel();
    let registry = JobRegistry::new(move |e| {
        if matches!(e, JobEvent::Finished { .. }) {
            let _ = tx.send(e);
        }
    });
    registry.spawn(|_| Ok(Report::default()));
    registry.spawn(|_| Err(Error::Cancelled));
    registry.spawn(|_| Err(Error::Pdf("el archivo está dañado".into())));
    for _ in 0..3 {
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
    }

    let text = std::fs::read_to_string(logs.join("pliego.log")).unwrap();
    assert!(text.contains("INFO  arranque"), "{text}");
    assert_eq!(text.matches(" falló: ").count(), 1, "{text}");
    assert!(text.contains("ERROR job"), "{text}");
    assert!(text.contains("el archivo está dañado"), "{text}");
}
