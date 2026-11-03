//! Hilo dedicado a un documento: abre el PDF dentro del hilo y ejecuta las peticiones que le llegan.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, mpsc};
use std::thread::JoinHandle;

use pliego_core::{Error, PdfEngine, Result};
use pliego_engine_mupdf::{MupdfDocument, MupdfEngine};

type Job = Box<dyn FnOnce(&MupdfDocument) + Send>;

pub struct DocWorker {
    tx: Mutex<Option<Sender<Job>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    revision: AtomicU64,
}

impl DocWorker {
    pub fn spawn(path: PathBuf, password: Option<String>) -> Result<Self> {
        let (tx, rx) = channel::<Job>();
        let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<()>>(1);
        let thread = std::thread::Builder::new()
            .name("pliego-doc".into())
            .spawn(move || {
                let doc = match MupdfEngine.open(&path, password.as_deref()) {
                    Ok(d) => {
                        let _ = ready_tx.send(Ok(()));
                        d
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                while let Ok(job) = rx.recv() {
                    job(&doc);
                }
            })
            .map_err(Error::from)?;
        ready_rx
            .recv()
            .map_err(|_| Error::Other("el hilo del documento terminó antes de abrirlo".into()))??;
        Ok(Self {
            tx: Mutex::new(Some(tx)),
            thread: Mutex::new(Some(thread)),
            revision: AtomicU64::new(0),
        })
    }

    /// Ejecuta `f` en el hilo del documento y espera el resultado.
    pub fn call<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&MupdfDocument) -> Result<T> + Send + 'static,
    {
        let (rtx, rrx) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |d| {
            // Un fallo de MuPDF no debe tirar la app: un pánico se convierte en error.
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(d)))
                .unwrap_or_else(|_| Err(Error::Pdf("el motor falló al procesar el documento".into())));
            let _ = rtx.send(r);
        });
        {
            let guard = self.tx.lock().unwrap_or_else(|e| e.into_inner());
            let tx = guard.as_ref().ok_or_else(|| Error::Other("documento cerrado".into()))?;
            tx.send(job)
                .map_err(|_| Error::Other("el hilo del documento ya no responde".into()))?;
        }
        rrx.recv()
            .map_err(|_| Error::Other("el hilo del documento ya no responde".into()))?
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    pub fn bump_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst) + 1
    }
}

impl Drop for DocWorker {
    fn drop(&mut self) {
        // Cerrar el canal hace terminar el bucle; esperamos para liberar MuPDF antes de seguir.
        self.tx.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(t) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = t.join();
        }
    }
}
