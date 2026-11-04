//! Hilo de actualizaciones de la app: espera a la hora de comprobar, ejecuta las órdenes de la interfaz y avisa de
//! cada cambio. No sabe nada de Tauri: la app le da una función `on_change` que emite el evento
//! `update-state-changed`.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use pliego_core::{CancelToken, JobContext, Progress};
use serde::Serialize;
use time::OffsetDateTime;

use crate::config::UpdateConfig;
use crate::service::{Phase, Stager, Status, UpdateService};

/// Lo que ve la interfaz: el estado del servicio y si hay trabajo en curso (comprobando o descargando).
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    #[serde(flatten)]
    pub status: Status,
    pub busy: bool,
}

enum Command {
    CheckNow,
    Download,
    Skip,
    DismissNotices,
    DismissRollback,
    Settings {
        config: Box<UpdateConfig>,
        respect_metered: bool,
    },
    Stop,
}

type Notify = Arc<dyn Fn(&Snapshot) + Send + Sync>;

struct Shared {
    snapshot: Mutex<Snapshot>,
    queue: Mutex<VecDeque<Command>>,
    wake: Condvar,
    cancel: CancelToken,
    notify: Notify,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Shared {
    fn publish(&self, status: Status, busy: bool) {
        let snap = Snapshot { status, busy };
        *lock(&self.snapshot) = snap.clone();
        (self.notify)(&snap);
    }

    /// Espera una orden hasta `due`; `None` si llegó la hora de comprobar (o `Stop` ya consumido).
    fn next(&self, due: Option<OffsetDateTime>) -> Option<Command> {
        let mut q = lock(&self.queue);
        loop {
            if let Some(c) = q.pop_front() {
                return Some(c);
            }
            q = match due {
                None => self.wake.wait(q).unwrap_or_else(|e| e.into_inner()),
                Some(t) => {
                    let left = t - OffsetDateTime::now_utc();
                    let Ok(left) = Duration::try_from(left) else {
                        return None;
                    };
                    if left.is_zero() {
                        return None;
                    }
                    self.wake.wait_timeout(q, left).unwrap_or_else(|e| e.into_inner()).0
                }
            };
        }
    }
}

/// Convierte el progreso de la descarga en cambios de estado.
struct Live<'a>(&'a Shared);

impl Progress for Live<'_> {
    fn report(&self, done: u64, total: u64, _message: &str) {
        let mut snap = lock(&self.0.snapshot).clone();
        snap.status.phase = Phase::Downloading { done, total };
        snap.busy = true;
        *lock(&self.0.snapshot) = snap.clone();
        (self.0.notify)(&snap);
    }
}

pub struct Runner {
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Runner {
    /// Arranca el hilo. `is_metered`: se consulta en cada ciclo (la conexión puede cambiar). `respect_metered`: el
    /// ajuste "con conexión de uso medido, solo comprobar".
    pub fn start<S>(
        service: UpdateService<S>,
        respect_metered: bool,
        is_metered: impl Fn() -> bool + Send + 'static,
        on_change: impl Fn(&Snapshot) + Send + Sync + 'static,
    ) -> Self
    where
        S: Stager + Send + 'static,
    {
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(Snapshot {
                status: service.status(),
                busy: false,
            }),
            queue: Mutex::new(VecDeque::new()),
            wake: Condvar::new(),
            cancel: CancelToken::new(),
            notify: Arc::new(on_change),
        });
        let worker = shared.clone();
        let thread = std::thread::Builder::new()
            .name("pliego-updates".into())
            .spawn(move || work(service, respect_metered, &is_metered, &worker))
            .ok();
        Self {
            shared,
            thread: Mutex::new(thread),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.shared.snapshot).clone()
    }

    fn send(&self, c: Command) {
        lock(&self.shared.queue).push_back(c);
        self.shared.wake.notify_all();
    }

    /// "Buscar ahora".
    pub fn check_now(&self) {
        self.send(Command::CheckNow);
    }

    /// "Descargar" (modo "solo avisar" o conexión medida).
    pub fn download(&self) {
        self.send(Command::Download);
    }

    /// "Omitir esta versión".
    pub fn skip_version(&self) {
        self.send(Command::Skip);
    }

    /// Cierra la tarjeta de novedades o el aviso de vuelta atrás.
    pub fn dismiss_notices(&self) {
        self.send(Command::DismissNotices);
    }

    /// Cierra solo el aviso de vuelta atrás.
    pub fn dismiss_rollback(&self) {
        self.send(Command::DismissRollback);
    }

    /// Modo, canal o directiva cambiados.
    pub fn apply_settings(&self, config: UpdateConfig, respect_metered: bool) {
        self.send(Command::Settings {
            config: Box::new(config),
            respect_metered,
        });
    }

    /// Cancela lo que esté en curso y termina el hilo.
    pub fn stop(&self) {
        self.shared.cancel.cancel();
        self.send(Command::Stop);
        if let Some(t) = lock(&self.thread).take() {
            let _ = t.join();
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        self.stop();
    }
}

fn work<S: Stager>(
    mut svc: UpdateService<S>,
    mut respect_metered: bool,
    is_metered: &dyn Fn() -> bool,
    shared: &Shared,
) {
    loop {
        let cmd = shared.next(svc.next_check_at());
        let live = Live(shared);
        let ctx = JobContext::new(&live, shared.cancel.clone());
        let busy = |svc: &UpdateService<S>| shared.publish(svc.status(), true);
        match cmd {
            Some(Command::Stop) => return,
            None => {
                busy(&svc);
                svc.run_cycle(respect_metered && is_metered(), &ctx);
            }
            Some(Command::CheckNow) => {
                busy(&svc);
                svc.run_cycle(respect_metered && is_metered(), &ctx);
            }
            Some(Command::Download) => {
                busy(&svc);
                svc.download(&ctx);
            }
            Some(Command::Skip) => {
                let _ = svc.skip_version();
            }
            Some(Command::DismissNotices) => {
                let _ = svc.dismiss_notices();
            }
            Some(Command::DismissRollback) => {
                let _ = svc.dismiss_rollback();
            }
            Some(Command::Settings {
                config,
                respect_metered: r,
            }) => {
                respect_metered = r;
                svc.set_config(*config);
            }
        }
        shared.publish(svc.status(), false);
    }
}
