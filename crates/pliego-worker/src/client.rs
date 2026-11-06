//! Cliente del worker: `WorkerEngine` implementa los traits de `pliego-core`.

use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pliego_core::{
    DocInfo, EngineDocument, Error, Layer, OutlineItem, PageSize, PageWord, PdfEngine, Result, RgbaImage, SearchHit,
    TileRequest,
};

use crate::cache::{TileCache, TileKey};
use crate::platform::Sandbox;
use crate::protocol::{MAX_BINARY_FRAME, MAX_JSON_FRAME, Request, Response, read_frame, write_frame, write_json};

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Ejecutable `pliego-worker`.
    pub exe: PathBuf,
    /// Tiempo máximo por petición antes de dar el worker por colgado y relanzarlo.
    pub timeout: Duration,
    /// Memoria máxima de la caché de teselas. Está en el proceso de la app, dentro de los 150 MB de Pliego: 64 MB
    /// bastan para la vista y sus vecinas y volver a dibujar una tesela cuesta unos milisegundos.
    pub cache_bytes: usize,
    /// Activa las peticiones de depuración del worker (solo pruebas).
    pub debug: bool,
    /// Límite de memoria del proceso worker (solo Windows, mediante job object). `None` = sin límite.
    pub memory_limit: Option<usize>,
    /// Tamaño (MB) desde el que el worker abre los archivos solo para verlos (menos memoria, sin editar capas hasta
    /// cierto tamaño). `None` = el predeterminado de `pliego_engine::VIEW_ONLY_MIN_BYTES`; `Some(0)` = siempre (pruebas).
    pub view_only_mb: Option<u64>,
}

impl WorkerConfig {
    pub fn new(exe: impl Into<PathBuf>) -> Self {
        Self {
            exe: exe.into(),
            timeout: Duration::from_secs(30),
            cache_bytes: 64 * 1024 * 1024,
            debug: false,
            memory_limit: Some(2 * 1024 * 1024 * 1024),
            view_only_mb: None,
        }
    }

    /// Busca `pliego-worker` en `PLIEGO_WORKER` o junto al ejecutable actual.
    pub fn discover() -> Result<Self> {
        if let Some(p) = std::env::var_os("PLIEGO_WORKER") {
            return Ok(Self::new(p));
        }
        let me = std::env::current_exe()?;
        let name = format!("pliego-worker{}", std::env::consts::EXE_SUFFIX);
        let exe = me.with_file_name(name);
        if exe.exists() {
            Ok(Self::new(exe))
        } else {
            Err(Error::Other(format!("no se encuentra el proceso auxiliar {exe:?}")))
        }
    }
}

pub struct WorkerEngine {
    config: Arc<WorkerConfig>,
    cache: Arc<TileCache>,
    next_id: AtomicU64,
}

impl WorkerEngine {
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            cache: Arc::new(TileCache::new(config.cache_bytes)),
            config: Arc::new(config),
            next_id: AtomicU64::new(1),
        }
    }

    /// Caché de teselas compartida por los documentos de este motor.
    pub fn cache(&self) -> &Arc<TileCache> {
        &self.cache
    }

    fn open_source(&self, source: Source, password: Option<&str>) -> Result<WorkerDocument> {
        let doc = WorkerDocument {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            config: self.config.clone(),
            cache: self.cache.clone(),
            revision: AtomicU64::new(0),
            restarts: AtomicU32::new(0),
            recovered: AtomicBool::new(false),
            state: Mutex::new(State {
                proc: None,
                source,
                password: password.map(str::to_owned),
                hidden_layers: Vec::new(),
            }),
        };
        // Abre ya: así los errores de contraseña o de formato salen en `open`.
        doc.with_proc(|_| Ok(()))?;
        Ok(doc)
    }
}

impl PdfEngine for WorkerEngine {
    type Doc = WorkerDocument;

    fn open(&self, path: &Path, password: Option<&str>) -> Result<WorkerDocument> {
        if !path.exists() {
            return Err(Error::io_at(path, io::Error::from(io::ErrorKind::NotFound)));
        }
        let abs = std::path::absolute(path).map_err(|e| Error::io_at(path, e))?;
        self.open_source(Source::Path(abs), password)
    }

    fn open_bytes(&self, bytes: &[u8], password: Option<&str>) -> Result<WorkerDocument> {
        self.open_source(Source::from_bytes(bytes, None)?, password)
    }
}

/// Desde este tamaño, un contenido que llega en memoria (`reload_bytes`, `open_bytes`) se pasa al worker por archivo
/// temporal: no se queda una copia entera en la app, no viaja por la tubería y no tropieza con el límite de marco.
const BYTES_TO_FILE_MIN: usize = 16 * 1024 * 1024;

#[derive(Clone)]
enum Source {
    Path(PathBuf),
    Bytes(Arc<Vec<u8>>),
    /// Copia temporal del contenido; se borra al soltar el último `Arc` (al recargar o cerrar el documento).
    Temp(Arc<tempfile::TempPath>),
}

impl Source {
    /// Guarda `bytes` donde corresponda. `beside`: archivo del documento, para dejar el temporal en su misma unidad.
    fn from_bytes(bytes: &[u8], beside: Option<&Path>) -> Result<Source> {
        if bytes.len() < BYTES_TO_FILE_MIN {
            return Ok(Source::Bytes(Arc::new(bytes.to_vec())));
        }
        let mut b = tempfile::Builder::new();
        b.prefix(".pliego-vista-").suffix(".pdf");
        let write = |dir: Option<&Path>| -> io::Result<tempfile::TempPath> {
            let mut f = match dir {
                Some(d) => b.tempfile_in(d)?,
                None => b.tempfile()?,
            };
            f.write_all(bytes)?;
            f.flush()?;
            Ok(f.into_temp_path())
        };
        let dir = beside.and_then(Path::parent).filter(|p| !p.as_os_str().is_empty());
        let tmp = write(dir)
            .or_else(|_| write(None))
            .map_err(|e| Error::Other(format!("no se pudo preparar el documento para el visor: {e}")))?;
        Ok(Source::Temp(Arc::new(tmp)))
    }

    /// Archivo de este documento, si es un archivo (el original o la copia temporal).
    fn file(&self) -> Option<&Path> {
        match self {
            Source::Path(p) => Some(p),
            Source::Temp(t) => Some(t),
            Source::Bytes(_) => None,
        }
    }
}

struct Proc {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    /// Marcos que lee un hilo desde stdout del worker; `Err` cuando el flujo se corta.
    rx: Receiver<io::Result<Vec<u8>>>,
    /// Job object (Windows): se cierra después de matar al proceso.
    _sandbox: Option<Sandbox>,
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = write_json(&mut self.stdin, &Request::Shutdown);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct State {
    proc: Option<Proc>,
    source: Source,
    password: Option<String>,
    /// Capas ocultas por el usuario: se reaplican si el worker se relanza o se recarga el contenido.
    hidden_layers: Vec<i32>,
}

/// Documento servido por un proceso worker propio. Es `Send + Sync`: las peticiones se serializan.
pub struct WorkerDocument {
    id: u64,
    config: Arc<WorkerConfig>,
    cache: Arc<TileCache>,
    revision: AtomicU64,
    restarts: AtomicU32,
    recovered: AtomicBool,
    state: Mutex<State>,
}

/// Fallo de transporte (worker caído o colgado), distinto de un error de la operación.
enum CallError {
    Transport(String),
    /// El worker no respondió a tiempo: casi siempre es determinista (PDF patológico), no se reintenta.
    Timeout(String),
    Op(Error),
}

fn spawn(config: &WorkerConfig) -> io::Result<Proc> {
    let mut cmd = Command::new(&config.exe);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    if config.debug {
        cmd.env("PLIEGO_WORKER_DEBUG", "1");
    }
    if let Some(mb) = config.view_only_mb {
        cmd.env("PLIEGO_VIEW_ONLY_MB", mb.to_string());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | CREATE_SUSPENDED: `Sandbox::attach` lo mete en el job y lo reanuda.
        cmd.creation_flags(0x0800_0000 | crate::platform::windows::CREATE_SUSPENDED);
    }
    let mut child = cmd.spawn()?;
    // Si no se puede restringir el proceso no se usa: un worker sin límites no cumple el aislamiento.
    let sandbox = match Sandbox::attach(&child, config.memory_limit) {
        Ok(s) => s,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
    };
    let stdin = BufWriter::new(child.stdin.take().expect("stdin del worker"));
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout del worker"));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            match read_frame(&mut stdout, MAX_BINARY_FRAME.max(MAX_JSON_FRAME)) {
                Ok(Some(f)) => {
                    if tx.send(Ok(f)).is_err() {
                        return;
                    }
                }
                Ok(None) => {
                    let _ = tx.send(Err(io::Error::from(io::ErrorKind::UnexpectedEof)));
                    return;
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            }
        }
    });
    Ok(Proc {
        child,
        stdin,
        rx,
        _sandbox: sandbox,
    })
}

impl Proc {
    fn recv(&self, timeout: Duration) -> std::result::Result<Vec<u8>, CallError> {
        match self.rx.recv_timeout(timeout) {
            Ok(Ok(f)) => Ok(f),
            Ok(Err(e)) => Err(CallError::Transport(format!("el proceso del motor se cerró ({e})"))),
            Err(RecvTimeoutError::Timeout) => Err(CallError::Timeout("el motor tardó demasiado en responder".into())),
            Err(RecvTimeoutError::Disconnected) => Err(CallError::Transport("el proceso del motor se cerró".into())),
        }
    }

    fn call(
        &mut self,
        req: &Request,
        binary: Option<&[u8]>,
        timeout: Duration,
    ) -> std::result::Result<(Response, Option<Vec<u8>>), CallError> {
        let send = |p: &mut Proc| -> io::Result<()> {
            write_json(&mut p.stdin, req)?;
            if let Some(b) = binary {
                write_frame(&mut p.stdin, b)?;
            }
            p.stdin.flush()
        };
        send(self).map_err(|e| CallError::Transport(format!("no se pudo hablar con el motor ({e})")))?;
        let frame = self.recv(timeout)?;
        let resp: Response = serde_json::from_slice(&frame)
            .map_err(|e| CallError::Transport(format!("respuesta del motor no válida ({e})")))?;
        match resp {
            Response::Err(e) => Err(CallError::Op(e.into())),
            Response::Image { .. } => {
                let data = self.recv(timeout)?;
                Ok((resp, Some(data)))
            }
            other => Ok((other, None)),
        }
    }
}

impl WorkerDocument {
    /// Identificador del documento dentro de la caché.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Revisión usada en las claves de la caché de teselas.
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    /// Indica que el documento cambió: las teselas de otras revisiones dejan de ser válidas.
    pub fn set_revision(&self, revision: u64) {
        self.revision.store(revision, Ordering::Relaxed);
        self.cache.drop_stale(self.id, revision);
    }

    /// Sustituye el contenido por `bytes` (p. ej. tras una edición) con la nueva `revision`.
    pub fn reload_bytes(&self, bytes: &[u8], revision: u64) -> Result<()> {
        let mut st = self.lock();
        let beside = st.source.file().map(Path::to_path_buf);
        let source = Source::from_bytes(bytes, beside.as_deref())?;
        let old = (st.source.clone(), st.proc.take());
        st.source = source;
        drop(st);
        match self.with_proc(|_| Ok(())) {
            Ok(()) => {
                self.set_revision(revision);
                Ok(())
            }
            Err(e) => {
                let mut st = self.lock();
                st.source = old.0;
                st.proc = old.1;
                Err(e)
            }
        }
    }

    /// Identificador del proceso worker, si está en marcha (para medir su memoria desde fuera).
    pub fn worker_pid(&self) -> Option<u32> {
        self.lock().proc.as_ref().map(|p| p.child.id())
    }

    /// Número de veces que se relanzó el worker.
    pub fn restarts(&self) -> u32 {
        self.restarts.load(Ordering::Relaxed)
    }

    /// `true` una vez tras cada recuperación: la app lo usa para mostrar un aviso.
    pub fn take_recovered(&self) -> bool {
        self.recovered.swap(false, Ordering::Relaxed)
    }

    /// Deja colgado el worker sin esperar respuesta (requiere `WorkerConfig::debug`).
    #[doc(hidden)]
    pub fn debug_request_hang(&self) {
        if let Some(p) = self.lock().proc.as_mut() {
            let _ = write_json(&mut p.stdin, &Request::DebugHang).and_then(|_| p.stdin.flush());
        }
    }

    /// Mata el worker (pruebas de recuperación).
    #[doc(hidden)]
    pub fn kill_worker(&self) {
        if let Some(p) = self.lock().proc.as_mut() {
            let _ = p.child.kill();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn start(&self, st: &mut State) -> Result<()> {
        let mut proc = spawn(&self.config).map_err(|e| {
            pliego_core::log::error("worker", format!("no se pudo iniciar {:?}: {e}", self.config.exe));
            Error::Other(format!(
                "no se pudo iniciar el proceso del motor {:?}: {e}",
                self.config.exe
            ))
        })?;
        let pw = st.password.clone();
        let (req, bin) = match &st.source {
            Source::Bytes(b) => (Request::OpenBytes { password: pw }, Some(b.clone())),
            file => {
                let p = file.file().expect("Path y Temp son archivos");
                (
                    Request::OpenPath {
                        path: p
                            .to_str()
                            .ok_or_else(|| Error::InvalidInput(format!("la ruta {p:?} no es UTF-8 válido")))?
                            .to_owned(),
                        password: pw,
                    },
                    None,
                )
            }
        };
        match proc.call(&req, bin.as_deref().map(|b| b.as_slice()), self.config.timeout) {
            Ok(_) => {
                for &id in &st.hidden_layers {
                    // Una capa que ya no existe (contenido recargado) se ignora: no debe impedir abrir.
                    let _ = proc.call(&Request::SetLayer { id, visible: false }, None, self.config.timeout);
                }
                st.proc = Some(proc);
                Ok(())
            }
            Err(CallError::Op(e)) => Err(e),
            Err(CallError::Transport(m) | CallError::Timeout(m)) => Err(Error::Pdf(m)),
        }
    }

    /// Ejecuta `f` con un worker vivo, arrancándolo si hace falta.
    fn with_proc<T>(&self, f: impl FnOnce(&mut Proc) -> Result<T>) -> Result<T> {
        let mut st = self.lock();
        if st.proc.is_none() {
            self.start(&mut st)?;
        }
        f(st.proc.as_mut().expect("worker iniciado"))
    }

    /// Envía una petición; si el worker cae o se cuelga, lo relanza, reabre el documento y reintenta una vez.
    fn call(&self, req: Request) -> Result<(Response, Option<Vec<u8>>)> {
        let mut st = self.lock();
        for attempt in 0..2 {
            if st.proc.is_none() {
                self.start(&mut st)?;
            }
            let res = st
                .proc
                .as_mut()
                .expect("worker iniciado")
                .call(&req, None, self.config.timeout);
            match res {
                Ok(r) => return Ok(r),
                Err(CallError::Op(e)) => return Err(e),
                Err(err) => {
                    let (msg, retry) = match err {
                        CallError::Transport(m) => (m, attempt == 0),
                        CallError::Timeout(m) => (m, false),
                        CallError::Op(_) => unreachable!("tratado arriba"),
                    };
                    pliego_core::log::warn(
                        "worker",
                        format!(
                            "{msg} (intento {}; {})",
                            attempt + 1,
                            if retry {
                                "se relanza y se reintenta"
                            } else {
                                "se aísla"
                            }
                        ),
                    );
                    st.proc = None; // Drop mata el proceso; la siguiente petición lo relanza
                    self.restarts.fetch_add(1, Ordering::Relaxed);
                    self.recovered.store(true, Ordering::Relaxed);
                    if !retry {
                        return Err(Error::Pdf(format!(
                            "{msg}. El documento puede estar dañado o ser malicioso; se aisló sin afectar a la aplicación"
                        )));
                    }
                }
            }
        }
        unreachable!("el bucle devuelve en el segundo intento")
    }

    fn image(&self, req: Request) -> Result<RgbaImage> {
        match self.call(req)? {
            (Response::Image { width, height }, Some(data))
                if data.len() as u64 == width as u64 * height as u64 * 4 =>
            {
                Ok(RgbaImage { width, height, data })
            }
            _ => Err(Error::Pdf("el motor devolvió una imagen no válida".into())),
        }
    }

    /// Como `render_tile`, pero devuelve la tesela compartida sin copiarla y usa la caché.
    pub fn render_tile_cached(&self, req: TileRequest) -> Result<Arc<RgbaImage>> {
        let key = TileKey::new(self.id, self.revision(), req.page, req.zoom, req.col, req.row);
        if let Some(hit) = self.cache.get(&key) {
            return Ok(hit);
        }
        let img = Arc::new(self.image(Request::RenderTile(req))?);
        self.cache.insert(key, img.clone());
        Ok(img)
    }
}

impl Drop for WorkerDocument {
    fn drop(&mut self) {
        self.cache.drop_doc(self.id);
    }
}

fn unexpected<T>() -> Result<T> {
    Err(Error::Pdf("respuesta inesperada del motor".into()))
}

impl EngineDocument for WorkerDocument {
    fn info(&self) -> Result<DocInfo> {
        match self.call(Request::Info)?.0 {
            Response::Info(i) => Ok(i),
            _ => unexpected(),
        }
    }

    fn page_count(&self) -> Result<u32> {
        match self.call(Request::PageCount)?.0 {
            Response::Count(n) => Ok(n),
            _ => unexpected(),
        }
    }

    fn page_size(&self, page: u32) -> Result<PageSize> {
        match self.call(Request::PageSize { page })?.0 {
            Response::Size(s) => Ok(s),
            _ => unexpected(),
        }
    }

    fn render_page(&self, page: u32, zoom: f32) -> Result<RgbaImage> {
        self.image(Request::RenderPage { page, zoom })
    }

    fn render_tile(&self, req: TileRequest) -> Result<RgbaImage> {
        Ok((*self.render_tile_cached(req)?).clone())
    }

    fn page_text(&self, page: u32) -> Result<String> {
        match self.call(Request::PageText { page })?.0 {
            Response::Text(t) => Ok(t),
            _ => unexpected(),
        }
    }

    fn search_page(&self, page: u32, needle: &str, max_hits: u32) -> Result<SearchHit> {
        match self
            .call(Request::SearchPage {
                page,
                needle: needle.to_owned(),
                max_hits,
            })?
            .0
        {
            Response::Hit(h) => Ok(h),
            _ => unexpected(),
        }
    }

    fn page_words(&self, page: u32) -> Result<Vec<PageWord>> {
        match self.call(Request::PageWords { page })?.0 {
            Response::Words(w) => Ok(w),
            _ => unexpected(),
        }
    }

    fn outline(&self) -> Result<Vec<OutlineItem>> {
        match self.call(Request::Outline)?.0 {
            Response::Outline(o) => Ok(o),
            _ => unexpected(),
        }
    }

    fn layers(&self) -> Result<Vec<Layer>> {
        match self.call(Request::Layers)?.0 {
            Response::Layers(l) => Ok(l),
            _ => unexpected(),
        }
    }

    fn set_layer_visible(&self, id: i32, visible: bool) -> Result<()> {
        self.call(Request::SetLayer { id, visible })?;
        let mut st = self.lock();
        st.hidden_layers.retain(|&x| x != id);
        if !visible {
            st.hidden_layers.push(id);
        }
        drop(st);
        // Las teselas ya dibujadas muestran el estado anterior.
        self.cache.drop_doc(self.id);
        Ok(())
    }
}
