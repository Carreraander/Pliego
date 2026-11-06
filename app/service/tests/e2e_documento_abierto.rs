//! Recorrido de punta a punta de lo que hace la app con un documento **abierto en el worker**: comentar, añadir texto,
//! rellenar y diseñar campos escriben encima del propio PDF (deshacer incluido) y los trabajos archivo → archivo parten
//! de un PDF que el visor tiene abierto. Es la prueba que habría cazado el «Access is denied» de Windows (el worker
//! dejaba el archivo abierto y el guardado atómico no podía renombrar encima).
//!
//! Va en su propio ejecutable por lo mismo que `protect.rs`: el worker hereda los archivos abiertos por otros tests.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use pliego_app_service::annot::{AnnotSpec, Editor};
use pliego_app_service::{DocId, DocumentService};
use pliego_core::{JobContext, TileRequest};
use pliego_worker::WorkerConfig;

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

fn worker_exe() -> PathBuf {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        if let Some(p) = std::env::var_os("PLIEGO_WORKER") {
            return PathBuf::from(p);
        }
        let dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let exe = dir.join(format!("pliego-worker{}", std::env::consts::EXE_SUFFIX));
        if !exe.exists() {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let mut cmd = Command::new(cargo);
            cmd.args(["build", "-p", "pliego-worker"]);
            if dir.file_name().is_some_and(|n| n == "release") {
                cmd.arg("--release");
            }
            assert!(cmd.status().unwrap().success(), "no se pudo compilar pliego-worker");
        }
        exe
    })
    .clone()
}

fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn service() -> DocumentService {
    let mut c = WorkerConfig::new(worker_exe());
    c.timeout = std::time::Duration::from_secs(120);
    DocumentService::new(c)
}

/// Servicio cuyo worker abre todo «solo para ver» (así abre la app los PDF grandes).
fn view_only_service() -> DocumentService {
    let mut c = WorkerConfig::new(worker_exe());
    c.timeout = std::time::Duration::from_secs(120);
    c.view_only_mb = Some(0);
    DocumentService::new(c)
}

/// Copia un PDF del corpus a una carpeta temporal y lo abre en el worker, como hace la app al abrir un archivo.
fn open(rel: &str) -> (tempfile::TempDir, PathBuf, DocumentService, DocId) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(Path::new(rel).file_name().unwrap());
    std::fs::copy(corpus(rel), &path).unwrap();
    let svc = service();
    let id = svc.open_document(&path, None).unwrap();
    (dir, path, svc, id)
}

/// Lo que ve la persona tras un cambio: la revisión sube y la página se vuelve a pintar (PNG válido).
fn assert_repaints(svc: &DocumentService, id: DocId, before: u64) {
    assert!(svc.revision(id).unwrap() > before, "la revisión debe subir");
    let png = svc
        .tile(
            id,
            TileRequest {
                page: 0,
                zoom: 1.0,
                col: 0,
                row: 0,
                size: 512,
            },
        )
        .unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
}

fn pages_of(path: &Path) -> u32 {
    let svc = service();
    let id = svc.open_document(path, None).unwrap();
    svc.document_info(id).unwrap().page_count
}

#[test]
fn comentar_resaltar_y_deshacer_con_el_documento_abierto() {
    let _s = serial();
    let (dir, path, svc, id) = open("basic/hola.pdf");
    let original = std::fs::read(&path).unwrap();
    let ed = Editor::default();

    // Resaltar: la selección del visor llega como cuadriláteros de una búsqueda de texto.
    let hits = svc.search(id, "Hola", 5).unwrap();
    assert!(!hits.is_empty(), "el texto de prueba debe encontrarse");
    let quads = hits[0].quads.clone();
    let mut spec = AnnotSpec::new(hits[0].page as usize, pliego_annot::Geometry::Highlight { quads });
    spec.author = Some("Ander".into());
    let before = svc.revision(id).unwrap();
    let bytes = ed.add(&path, None, &spec, "copia de seguridad").unwrap();
    svc.reload_bytes(id, &bytes).unwrap();
    assert_repaints(&svc, id, before);
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "el PDF abierto quedó guardado");
    assert_eq!(
        std::fs::read(dir.path().join("hola (copia de seguridad).pdf")).unwrap(),
        original,
        "copia de seguridad idéntica al original"
    );
    let summary = pliego_app_service::annot::summary_of(&path, None).unwrap();
    assert_eq!(summary.total, 1);

    // Nota, texto libre y dibujo siguen apilando cambios sobre el mismo archivo abierto.
    for geometry in [
        pliego_annot::Geometry::Note { at: [100.0, 100.0] },
        pliego_annot::Geometry::FreeText {
            rect: [60.0, 600.0, 300.0, 640.0],
            font_size: 12.0,
        },
    ] {
        let mut s = AnnotSpec::new(0, geometry);
        s.contents = Some("que tal estas?".into());
        let rev = svc.revision(id).unwrap();
        let b = ed.add(&path, None, &s, "copia de seguridad").unwrap();
        svc.reload_bytes(id, &b).unwrap();
        assert_repaints(&svc, id, rev);
    }
    assert_eq!(pliego_app_service::annot::summary_of(&path, None).unwrap().total, 3);
    assert_eq!(ed.undo_depth(&path), 3);

    // Deshacer tres veces devuelve el archivo original byte a byte.
    for _ in 0..3 {
        let b = ed.undo(&path).unwrap().unwrap();
        svc.reload_bytes(id, &b).unwrap();
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(ed.undo_depth(&path), 0);
}

#[test]
fn borrar_un_comentario_con_el_documento_abierto() {
    let _s = serial();
    let (_dir, path, svc, id) = open("basic/hola.pdf");
    let ed = Editor::default();
    let mut s = AnnotSpec::new(0, pliego_annot::Geometry::Note { at: [100.0, 100.0] });
    s.contents = Some("borrar".into());
    let b = ed.add(&path, None, &s, "cs").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    let list = pliego_app_service::annot::summary_of(&path, None).unwrap();
    let annot = list.threads[0].comment.id;
    let b = ed.delete(&path, None, annot, "cs").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    assert_eq!(pliego_app_service::annot::summary_of(&path, None).unwrap().total, 0);
}

#[test]
fn editar_y_anadir_texto_con_el_documento_abierto() {
    let _s = serial();
    let (_dir, path, svc, id) = open("edit/parrafos.pdf");
    let ed = Editor::default();
    let style = pliego_edit::Style::default();

    let rev = svc.revision(id).unwrap();
    let out = pliego_app_service::pageedit::insert(
        &ed,
        &path,
        None,
        0,
        [60.0, 600.0, 300.0, 640.0],
        "Añadido aquí",
        &style,
        "copia de seguridad",
    )
    .unwrap();
    svc.reload_bytes(id, &out.bytes).unwrap();
    assert_repaints(&svc, id, rev);
    assert!(svc.page_text(id, 0).unwrap().contains("Añadido"));

    let rev = svc.revision(id).unwrap();
    let out = pliego_app_service::pageedit::replace(
        &ed,
        &path,
        None,
        0,
        0,
        "Texto nuevo",
        &style,
        true,
        "copia de seguridad",
    )
    .unwrap();
    svc.reload_bytes(id, &out.bytes).unwrap();
    assert_repaints(&svc, id, rev);
    assert!(svc.page_text(id, 0).unwrap().contains("Texto nuevo"));
    assert_eq!(ed.undo_depth(&path), 2);
}

#[test]
fn rellenar_y_disenar_formularios_con_el_documento_abierto() {
    let _s = serial();
    let (_dir, path, svc, id) = open("organize/form1.pdf");
    let ed = Editor::default();
    let fields = pliego_app_service::annot::form_fields(&path, None).unwrap();
    assert!(!fields.is_empty(), "form1.pdf tiene campos");
    let text_field = fields
        .iter()
        .find(|f| matches!(f.kind, pliego_forms::FieldKind::Text))
        .expect("un campo de texto");

    let mut values = pliego_forms::FieldValues::new();
    values.insert(text_field.name.clone(), pliego_forms::FieldValue::Text("Ander".into()));
    let rev = svc.revision(id).unwrap();
    let b = ed.fill(&path, None, &values, "copia de seguridad").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    assert_repaints(&svc, id, rev);
    let after = pliego_app_service::annot::form_fields(&path, None).unwrap();
    assert_eq!(
        after
            .iter()
            .find(|f| f.name == text_field.name)
            .unwrap()
            .value
            .as_deref(),
        Some("Ander")
    );

    // Diseñar: añadir, cambiar y borrar un campo en el PDF abierto.
    let spec: pliego_formdesign::FieldSpec = serde_json::from_value(serde_json::json!({
        "name": "nuevo", "kind": "text", "page": 0, "rect": [50.0, 50.0, 200.0, 80.0]
    }))
    .unwrap();
    let b = pliego_app_service::formdesign::add(&ed, &path, None, &spec, "copia de seguridad").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    let designed = pliego_app_service::formdesign::fields(&path, None).unwrap();
    assert!(designed.iter().any(|f| f.name == "nuevo"));
    let patch: pliego_formdesign::FieldPatch = serde_json::from_value(serde_json::json!({ "required": true })).unwrap();
    let b = pliego_app_service::formdesign::update(&ed, &path, None, "nuevo", &patch, "copia de seguridad").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    let b = pliego_app_service::formdesign::delete(&ed, &path, None, "nuevo", "copia de seguridad").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    let designed = pliego_app_service::formdesign::fields(&path, None).unwrap();
    assert!(!designed.iter().any(|f| f.name == "nuevo"));
}

#[test]
fn trabajos_archivo_a_archivo_desde_un_pdf_abierto() {
    let _s = serial();
    use pliego_app_service::{optimize, organize, protect, stamp};
    let (dir, path, _svc, _id) = open("organize/a.pdf"); // abierto en el worker mientras corren los trabajos
    let original = std::fs::read(&path).unwrap();
    let n = pages_of(&path);
    let ctx = JobContext::silent();

    let compressed = optimize::compress_copy(
        &path,
        &optimize::CompressRequest {
            preset: "ebook".into(),
            suffix: "comprimido".into(),
            ..Default::default()
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(pages_of(&compressed.outputs[0]), n);

    let other = dir.path().join("otro.pdf");
    std::fs::copy(corpus("organize/b.pdf"), &other).unwrap();
    let merged = organize::merge_copy(&[path.clone(), other.clone()], &Default::default(), "combinado", &ctx).unwrap();
    assert_eq!(pages_of(&merged.outputs[0]), n + pages_of(&other));

    let split = organize::split_copy(
        &path,
        None,
        &pliego_organize::SplitRule::EveryN { n: 1 },
        "partes",
        &ctx,
    )
    .unwrap();
    assert_eq!(split.outputs.len() as u32, n);

    let numbered = organize::number_copy(
        &path,
        None,
        &organize::NumberRequest {
            position: organize::NumberPosition::FooterCenter,
            format: "Página {page} de {pages}".into(),
            from_page: 0,
            size: 10.0,
            also_labels: false,
        },
        "numerado",
        &ctx,
    )
    .unwrap();
    let svc2 = service();
    let nid = svc2.open_document(&numbered.outputs[0], None).unwrap();
    assert!(
        svc2.page_text(nid, 0).unwrap().contains("Página 1 de"),
        "el número se imprime en la hoja"
    );

    let marked = stamp::stamp_copy(
        &path,
        "all",
        None,
        &stamp::StampOp::Watermark(
            serde_json::from_value(serde_json::json!({
                "content": { "kind": "text", "text": "CONFIDENCIAL", "style": serde_json::to_value(pliego_stamp::TextStyle::default()).unwrap() }
            }))
            .unwrap(),
        ),
        &ctx,
    )
    .unwrap();
    let mid = svc2.open_document(&marked.outputs[0], None).unwrap();
    assert!(svc2.page_text(mid, 0).unwrap().contains("CONFIDENCIAL"));

    // Proteger → el visor pide contraseña → con ella se abre.
    let req: protect::ProtectRequest = serde_json::from_value(serde_json::json!({
        "path": path, "userPassword": "abre", "ownerPassword": "manda"
    }))
    .unwrap();
    let enc = protect::protect(&req, &ctx).unwrap().outputs.remove(0);
    assert!(matches!(
        svc2.open_document(&enc, None),
        Err(pliego_core::Error::Password)
    ));
    let eid = svc2.open_document(&enc, Some("abre")).unwrap();
    assert_eq!(svc2.document_info(eid).unwrap().page_count, n);

    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "ningún trabajo toca el original"
    );
}

#[test]
fn redactar_de_verdad_desde_un_pdf_abierto() {
    let _s = serial();
    use pliego_app_service::protect;
    let (_dir, path, svc, id) = open("redact/datos.pdf");
    let ctx = JobContext::silent();
    let found = protect::find_matches(&path, None, &["email".to_string()]).unwrap();
    assert!(!found.is_empty(), "el corpus tiene correos");
    let before_text = svc.page_text(id, 0).unwrap();
    let req: protect::RedactRequest = serde_json::from_value(serde_json::json!({
        "path": path, "patterns": ["email"]
    }))
    .unwrap();
    let out = protect::redact(&req, &ctx).unwrap().outputs.remove(0);
    let svc2 = service();
    let rid = svc2.open_document(&out, None).unwrap();
    let after_text = svc2.page_text(rid, 0).unwrap();
    assert!(before_text.contains('@'));
    assert!(
        !after_text.contains('@'),
        "el correo ya no se puede extraer: {after_text}"
    );
}

#[test]
fn convertir_comprobar_y_buscar_desde_un_pdf_abierto() {
    let _s = serial();
    use pliego_app_service::{checks, convert, portfolio, search_index};
    let (dir, path, svc, _id) = open("basic/hola.pdf");
    let ctx = JobContext::silent();
    let original = std::fs::read(&path).unwrap();

    // Exportar a imágenes y a texto: copias nuevas junto al original.
    let png = convert::export_copy(
        &path,
        &convert::ExportRequest {
            format: "png".into(),
            pages: "all".into(),
            dpi: Some(72.0),
            label: "imágenes".into(),
            ..Default::default()
        },
        &ctx,
    )
    .unwrap();
    assert!(!png.outputs.is_empty());
    let txt = convert::export_copy(
        &path,
        &convert::ExportRequest {
            format: "txt".into(),
            pages: "all".into(),
            label: "texto".into(),
            ..Default::default()
        },
        &ctx,
    )
    .unwrap();
    assert!(
        std::fs::read_to_string(&txt.outputs[0])
            .unwrap()
            .contains("Hola Pliego")
    );

    // Crear un PDF desde un .txt y abrirlo en el visor.
    let nota = dir.path().join("nota.txt");
    std::fs::write(&nota, "Hola desde Pliego").unwrap();
    let created = convert::create_copy(
        &[nota],
        &convert::CreateRequest {
            page_size: "a4".into(),
            orientation: "auto".into(),
            label: "PDF".into(),
            ..Default::default()
        },
        &ctx,
    )
    .unwrap();
    let cid = svc.open_document(&created.outputs[0], None).unwrap();
    assert!(svc.page_text(cid, 0).unwrap().contains("Hola desde Pliego"));

    // Comprobaciones de solo lectura.
    assert!(checks::accessibility(&path, None).is_ok());
    assert!(checks::preflight(&path, "basic", None).is_ok());
    assert!(checks::pdfa(&path, checks::Level::A2b, None).is_ok());

    // Cartera: crear con dos archivos, listar y extraer.
    let other = dir.path().join("otro.pdf");
    std::fs::copy(corpus("organize/b.pdf"), &other).unwrap();
    let port = portfolio::create_portfolio(&[path.clone(), other], "cartera", Default::default(), &ctx).unwrap();
    let listing = portfolio::list(&port.outputs[0], None).unwrap();
    assert!(listing.entries.len() >= 2, "{listing:?}");
    let extracted = portfolio::extract_copy(&port.outputs[0], &[], None, "adjuntos", &ctx).unwrap();
    assert!(!extracted.outputs.is_empty());

    // Índice de búsqueda sobre la carpeta con los PDF y consulta.
    let data = tempfile::tempdir().unwrap();
    search_index::index_folder(data.path(), dir.path(), &ctx).unwrap();
    let hits = search_index::search(data.path(), "Pliego", 10).unwrap();
    assert!(!hits.is_empty(), "el índice encuentra «Pliego»");

    assert_eq!(std::fs::read(&path).unwrap(), original, "nada de esto toca el original");
}

/// OCR con la CLI real (`pliego`), como la lanza la app. Necesita `pliego` compilado y el modelo `eng`; si falta algo,
/// se salta (en acr12 se ejecuta con `PLIEGO_CLI` apuntando a la CLI de la compilación).
#[test]
fn ocr_con_la_cli_real_desde_un_pdf_abierto() {
    let _s = serial();
    use pliego_app_service::ocr;
    let Ok(cli) = ocr::find_cli().or_else(|_| {
        let dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let exe = dir.join(format!("pliego{}", std::env::consts::EXE_SUFFIX));
        exe.is_file().then_some(exe).ok_or(())
    }) else {
        eprintln!("sin CLI `pliego`: prueba de OCR omitida");
        return;
    };
    let langs = ocr::languages(&cli).unwrap();
    if !langs.installed.iter().any(|l| l == "eng") {
        eprintln!("sin el modelo eng: prueba de OCR omitida");
        return;
    }
    assert!(langs.downloadable.iter().any(|l| l == "spa"));
    let (_dir, path, svc, _id) = open("basic/hola-ocr.pdf");
    let original = std::fs::read(&path).unwrap();
    let req = ocr::OcrRequest {
        path: path.clone(),
        langs: "eng".into(),
        force: true,
        ..Default::default()
    };
    let report = ocr::ocr_copy(&cli, &req, &JobContext::silent()).unwrap();
    let out = &report.outputs[0];
    assert!(out.file_name().unwrap().to_string_lossy().contains("(OCR)"));
    assert_eq!(std::fs::read(&path).unwrap(), original, "el original no cambia");
    let rid = svc.open_document(out, None).unwrap();
    assert!(
        !svc.page_text(rid, 0).unwrap().trim().is_empty(),
        "el PDF resultante tiene texto"
    );
}

/// PDF grande: el worker lo lee del archivo (sin manejador abierto), el historial de deshacer va a disco y cada edición
/// puede sustituir el archivo con el documento abierto. Es el recorrido que no podía fallar en Windows.
#[test]
fn pdf_grande_editar_y_deshacer_sin_memoria_ni_archivos_abiertos() {
    use pliego_app_service::history::Limits;
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hola.pdf");
    std::fs::copy(corpus("basic/hola.pdf"), &path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let svc = view_only_service();
    let id = svc.open_document(&path, None).unwrap();
    // `mem_step: 0` manda todos los pasos a disco, como harían los de un PDF de cientos de MB.
    let ed = Editor::with_history_limits(Limits {
        mem_step: 0,
        ..Limits::default()
    });
    let spilled = || {
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".pliego-deshacer-")
            })
            .count()
    };

    for _ in 0..2 {
        let mut s = AnnotSpec::new(0, pliego_annot::Geometry::Note { at: [100.0, 100.0] });
        s.contents = Some("nota".into());
        let rev = svc.revision(id).unwrap();
        let b = ed.add(&path, None, &s, "copia de seguridad").unwrap();
        svc.reload_bytes(id, &b).unwrap();
        assert_repaints(&svc, id, rev);
    }
    assert_eq!(ed.undo_depth(&path), 2);
    assert_eq!(spilled(), 2, "cada paso es un temporal junto al documento, no memoria");
    for _ in 0..2 {
        let b = ed.undo(&path).unwrap().unwrap();
        svc.reload_bytes(id, &b).unwrap();
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(spilled(), 0);
}

#[test]
fn pdf_grande_comentar_en_modo_ligero_no_reescribe_el_archivo() {
    use pliego_app_service::annot::{Saved, summary_of};
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hola.pdf");
    std::fs::copy(corpus("basic/hola.pdf"), &path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let svc = view_only_service();
    let id = svc.open_document(&path, None).unwrap();
    // `with_lean_min_bytes(0)`: todo se trata como un PDF grande.
    let ed = Editor::default().with_lean_min_bytes(0);
    let mut s = AnnotSpec::new(0, pliego_annot::Geometry::Note { at: [100.0, 100.0] });
    s.contents = Some("nota grande".into());
    let rev = svc.revision(id).unwrap();
    let saved = ed.add_saved(&path, None, &s, "copia de seguridad").unwrap();
    assert!(matches!(saved, Saved::File(_)), "el resultado se entrega como archivo");
    svc.reload_saved(id, &path, &saved).unwrap();
    assert_repaints(&svc, id, rev);
    let after = std::fs::read(&path).unwrap();
    assert!(
        after.starts_with(&original),
        "se añade al final: el original queda intacto"
    );
    assert_eq!(summary_of(&path, None).unwrap().total, 1);
    assert_eq!(ed.undo_depth(&path), 1);
    // Segundo comentario encima del primero y dos «deshacer» seguidos: cada uno solo corta lo que añadió su paso.
    let first_len = after.len();
    let saved = ed.add_saved(&path, None, &s, "copia de seguridad").unwrap();
    svc.reload_saved(id, &path, &saved).unwrap();
    assert!(std::fs::read(&path).unwrap().starts_with(&after));
    assert_eq!(summary_of(&path, None).unwrap().total, 2);
    let saved = ed.undo_saved(&path).unwrap().unwrap();
    svc.reload_saved(id, &path, &saved).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len() as usize, first_len);
    assert_eq!(summary_of(&path, None).unwrap().total, 1);
    let rev = svc.revision(id).unwrap();
    let saved = ed.undo_saved(&path).unwrap().unwrap();
    svc.reload_saved(id, &path, &saved).unwrap();
    assert_repaints(&svc, id, rev);
    assert_eq!(std::fs::read(&path).unwrap(), original);

    // Otro programa añade algo al archivo: deshacer ya no sabe dónde cortar y lo deja como está.
    ed.add_saved(&path, None, &s, "copia de seguridad").unwrap();
    let mut changed = std::fs::read(&path).unwrap();
    changed.extend(b"\n% otro programa\n");
    std::fs::write(&path, &changed).unwrap();
    assert!(ed.undo_saved(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}
