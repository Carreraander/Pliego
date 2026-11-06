//! Contraseña en todas las herramientas (matriz `docs/qa/validacion-funciones.md`): con un PDF **cifrado abierto en el
//! worker**, las ediciones sobre el propio archivo, Combinar, Comparar con el segundo archivo cifrado, Insertar páginas
//! de otro PDF protegido y el OCR (CLI real) usan la contraseña y las copias siguen protegidas.
//!
//! Va en su propio ejecutable por lo mismo que `e2e_documento_abierto.rs`: el worker hereda los archivos abiertos.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use pliego_app_service::annot::{AnnotSpec, Editor};
use pliego_app_service::organize::{self, InsertSource, MergeOptions};
use pliego_app_service::protect::{self, ProtectRequest};
use pliego_app_service::{DocId, DocumentService, compare, formdesign, measure, ocr, pageedit, passwords};
use pliego_core::{Error, JobContext, TileRequest};
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

fn assert_paints(svc: &DocumentService, id: DocId) {
    let png = svc
        .tile(
            id,
            TileRequest {
                page: 0,
                zoom: 0.5,
                col: 0,
                row: 0,
                size: 512,
            },
        )
        .unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
}

/// Copia `rel` del corpus a `dir` y la cifra con `user` como contraseña de apertura; devuelve la ruta cifrada.
fn encrypted(dir: &Path, rel: &str, name: &str, user: &str) -> PathBuf {
    let plain = dir.join(format!("{name}-claro.pdf"));
    std::fs::copy(corpus(rel), &plain).unwrap();
    let req: ProtectRequest = serde_json::from_value(serde_json::json!({
        "path": plain, "userPassword": user, "ownerPassword": format!("{user}-dueño")
    }))
    .unwrap();
    let enc = protect::protect(&req, &JobContext::silent()).unwrap().outputs.remove(0);
    let out = dir.join(format!("{name}.pdf"));
    std::fs::rename(enc, &out).unwrap();
    std::fs::remove_file(plain).unwrap();
    out
}

/// El resultado sigue protegido: sin contraseña pide, con ella se abre y se pinta. Devuelve el número de páginas.
fn assert_still_protected(svc: &DocumentService, path: &Path, pw: &str) -> u32 {
    assert!(
        matches!(svc.open_document(path, None), Err(Error::Password)),
        "{} debería seguir protegido",
        path.display()
    );
    assert!(
        matches!(svc.open_document(path, Some("incorrecta")), Err(Error::Password)),
        "{} no debe abrirse con otra contraseña",
        path.display()
    );
    let id = svc.open_document(path, Some(pw)).unwrap();
    assert_paints(svc, id);
    svc.document_info(id).unwrap().page_count
}

#[test]
fn comentar_y_deshacer_sobre_el_propio_pdf_protegido() {
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let path = encrypted(dir.path(), "basic/hola.pdf", "hola", "abre");
    let svc = service();
    let id = svc.open_document(&path, Some("abre")).unwrap();
    let original = std::fs::read(&path).unwrap();
    let ed = Editor::default();

    let hits = svc.search(id, "Hola", 5).unwrap();
    assert!(
        !hits.is_empty(),
        "el texto de prueba se encuentra en el documento abierto"
    );
    let spec = AnnotSpec::new(
        hits[0].page as usize,
        pliego_annot::Geometry::Highlight {
            quads: hits[0].quads.clone(),
        },
    );

    // Sin contraseña (o con otra) no se toca nada.
    for bad in [None, Some("mala")] {
        assert!(matches!(
            ed.add(&path, bad, &spec, "copia de seguridad"),
            Err(Error::Password)
        ));
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(ed.undo_depth(&path), 0);
    assert!(!dir.path().join("hola (copia de seguridad).pdf").exists());

    // Con la contraseña recordada: se guarda, se recarga el visor y el archivo sigue protegido.
    let before = svc.revision(id).unwrap();
    let bytes = ed.add(&path, Some("abre"), &spec, "copia de seguridad").unwrap();
    svc.reload_bytes(id, &bytes).unwrap();
    assert!(svc.revision(id).unwrap() > before);
    assert_paints(&svc, id);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(
        std::fs::read(dir.path().join("hola (copia de seguridad).pdf")).unwrap(),
        original,
        "la copia de seguridad es el original (también cifrado)"
    );
    assert_eq!(
        pliego_app_service::annot::summary_of(&path, Some("abre"))
            .unwrap()
            .total,
        1
    );
    assert!(pliego_app_service::annot::summary_of(&path, None).is_err());
    assert_still_protected(&svc, &path, "abre");

    // Borrar el comentario y deshacer también funcionan.
    let summary = pliego_app_service::annot::summary_of(&path, Some("abre")).unwrap();
    let annot = summary.threads[0].comment.id;
    let b = ed.delete(&path, Some("abre"), annot, "cs").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    assert_eq!(
        pliego_app_service::annot::summary_of(&path, Some("abre"))
            .unwrap()
            .total,
        0
    );
    let b = ed.undo(&path).unwrap().unwrap();
    svc.reload_bytes(id, &b).unwrap();
    assert_eq!(
        pliego_app_service::annot::summary_of(&path, Some("abre"))
            .unwrap()
            .total,
        1
    );
    ed.undo(&path).unwrap().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn rellenar_y_disenar_formularios_en_un_pdf_protegido() {
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let path = encrypted(dir.path(), "organize/form1.pdf", "form", "abre");
    let svc = service();
    let id = svc.open_document(&path, Some("abre")).unwrap();
    let ed = Editor::default();

    assert!(
        pliego_app_service::annot::form_fields(&path, None).is_err(),
        "sin contraseña no se leen los campos"
    );
    let fields = pliego_app_service::annot::form_fields(&path, Some("abre")).unwrap();
    let text = fields
        .iter()
        .find(|f| matches!(f.kind, pliego_forms::FieldKind::Text))
        .expect("un campo de texto");
    let mut values = pliego_forms::FieldValues::new();
    values.insert(text.name.clone(), pliego_forms::FieldValue::Text("Ander".into()));
    assert!(matches!(
        ed.fill(&path, None, &values, "copia de seguridad"),
        Err(Error::Password)
    ));
    let b = ed.fill(&path, Some("abre"), &values, "copia de seguridad").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    assert_paints(&svc, id);
    // El filtro de pulsaciones y las acciones de los scripts también reciben la contraseña.
    assert!(pliego_app_service::annot::form_keystroke(&path, None, &text.name, "a", "", (0, 0)).is_err());
    assert!(
        pliego_app_service::annot::form_keystroke(&path, Some("abre"), &text.name, "a", "", (0, 0))
            .unwrap()
            .accepted
    );
    assert!(matches!(
        ed.run_action(&path, None, &text.name, "U", "cs"),
        Err(Error::Password)
    ));
    let after = pliego_app_service::annot::form_fields(&path, Some("abre")).unwrap();
    assert_eq!(
        after.iter().find(|f| f.name == text.name).unwrap().value.as_deref(),
        Some("Ander")
    );

    // Diseñar: añadir, cambiar y borrar un campo.
    let spec: pliego_formdesign::FieldSpec = serde_json::from_value(serde_json::json!({
        "name": "nuevo", "kind": "text", "page": 0, "rect": [50.0, 50.0, 200.0, 80.0]
    }))
    .unwrap();
    assert!(formdesign::fields(&path, None).is_err());
    assert!(formdesign::add(&ed, &path, None, &spec, "cs").is_err());
    let b = formdesign::add(&ed, &path, Some("abre"), &spec, "cs").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    assert!(
        formdesign::fields(&path, Some("abre"))
            .unwrap()
            .iter()
            .any(|f| f.name == "nuevo")
    );
    let patch: pliego_formdesign::FieldPatch = serde_json::from_value(serde_json::json!({ "required": true })).unwrap();
    let b = formdesign::update(&ed, &path, Some("abre"), "nuevo", &patch, "cs").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    let b = formdesign::delete(&ed, &path, Some("abre"), "nuevo", "cs").unwrap();
    svc.reload_bytes(id, &b).unwrap();
    assert!(
        !formdesign::fields(&path, Some("abre"))
            .unwrap()
            .iter()
            .any(|f| f.name == "nuevo")
    );
    assert_still_protected(&svc, &path, "abre");
}

#[test]
fn editar_texto_y_medir_en_un_pdf_protegido() {
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let path = encrypted(dir.path(), "edit/parrafos.pdf", "parrafos", "abre");
    let svc = service();
    let id = svc.open_document(&path, Some("abre")).unwrap();
    let ed = Editor::default();
    let style = pliego_edit::Style::default();

    assert!(pageedit::blocks(&path, 0, None).is_err());
    let blocks = pageedit::blocks(&path, 0, Some("abre")).unwrap();
    assert_eq!(blocks.len(), 2);
    assert!(
        pageedit::replace(&ed, &path, None, 0, 0, "x", &style, true, "cs").is_err(),
        "sin contraseña no se edita"
    );
    let out = pageedit::replace(
        &ed,
        &path,
        Some("abre"),
        0,
        0,
        "Texto nuevo del bloque",
        &style,
        true,
        "cs",
    )
    .unwrap();
    svc.reload_bytes(id, &out.bytes).unwrap();
    assert!(svc.page_text(id, 0).unwrap().contains("Texto nuevo"));
    let out = pageedit::insert(
        &ed,
        &path,
        Some("abre"),
        0,
        [60.0, 600.0, 300.0, 640.0],
        "Añadido aquí",
        &style,
        "cs",
    )
    .unwrap();
    svc.reload_bytes(id, &out.bytes).unwrap();
    assert!(svc.page_text(id, 0).unwrap().contains("Añadido"));

    // Medir: escala y medida en el propio PDF protegido.
    let scale = measure::ratio_scale(100.0, pliego_measure::Unit::M).unwrap();
    assert!(measure::page_scale(&path, None, 0).is_err());
    assert!(measure::page_scale(&path, Some("abre"), 0).unwrap().is_none());
    assert!(
        measure::save(
            &ed,
            &path,
            None,
            "cs",
            0,
            &scale,
            &[[100.0, 100.0], [200.0, 100.0]],
            false,
            pliego_core::Lang::Es,
        )
        .is_err()
    );
    let (bytes, m) = measure::save(
        &ed,
        &path,
        Some("abre"),
        "cs",
        0,
        &scale,
        &[[100.0, 100.0], [200.0, 100.0]],
        false,
        pliego_core::Lang::Es,
    )
    .unwrap();
    svc.reload_bytes(id, &bytes).unwrap();
    assert!(m.length > 0.0);
    assert!(measure::page_scale(&path, Some("abre"), 0).unwrap().is_some());
    assert_eq!(ed.undo_depth(&path), 3);
    assert_still_protected(&svc, &path, "abre");
}

#[test]
fn combinar_archivos_protegidos_cada_uno_con_su_contrasena() {
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let a = encrypted(dir.path(), "organize/a.pdf", "a", "clave-a");
    let b = encrypted(dir.path(), "organize/b.pdf", "b", "clave-b");
    let c = dir.path().join("c.pdf");
    std::fs::copy(corpus("organize/a.pdf"), &c).unwrap();
    let svc = service();
    let na = assert_still_protected(&svc, &a, "clave-a");
    let nb = assert_still_protected(&svc, &b, "clave-b");
    let nc = svc
        .document_info(svc.open_document(&c, None).unwrap())
        .unwrap()
        .page_count;
    let ctx = JobContext::silent();
    let inputs = [a.clone(), b.clone(), c.clone()];

    // Sin contraseñas, o con una sola común, falla: no hay una que valga para los dos.
    assert!(matches!(
        organize::merge_copy(&inputs, &MergeOptions::default(), "combinado", &ctx),
        Err(Error::Password)
    ));
    let common = MergeOptions {
        password: Some("clave-a".into()),
        ..Default::default()
    };
    assert!(matches!(
        organize::merge_copy(&inputs, &common, "combinado", &ctx),
        Err(Error::Password)
    ));
    assert!(
        !dir.path().join("a (combinado).pdf").exists(),
        "un fallo no deja una copia a medias"
    );

    // Con la contraseña de cada archivo (como las manda el panel por JSON) se combina.
    let options: MergeOptions = serde_json::from_value(serde_json::json!({
        "keep_bookmarks": true,
        "bookmark_per_file": true,
        "file_passwords": { a.to_string_lossy(): "clave-a", b.to_string_lossy(): "clave-b" }
    }))
    .unwrap();
    let report = organize::merge_copy(&inputs, &options, "combinado", &ctx).unwrap();
    let out = &report.outputs[0];
    let id = svc.open_document(out, None).unwrap();
    assert_eq!(svc.document_info(id).unwrap().page_count, na + nb + nc);
    assert_paints(&svc, id);
    // Los originales no cambian.
    assert_still_protected(&svc, &a, "clave-a");
}

#[test]
fn comparar_con_el_segundo_archivo_protegido() {
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let a = encrypted(dir.path(), "compare/base.pdf", "v1", "clave-a");
    let b = encrypted(dir.path(), "compare/palabra.pdf", "v2", "clave-b");
    let ctx = JobContext::silent();
    let req = |pa: Option<&str>, pb: Option<&str>| compare::CompareRequest {
        other: b.clone(),
        password_a: pa.map(Into::into),
        password_b: pb.map(Into::into),
        text: true,
        visual: true,
        label: "comparación".into(),
        lang: Default::default(),
    };

    for (pa, pb) in [(None, None), (Some("clave-a"), None), (Some("clave-a"), Some("mala"))] {
        assert!(
            compare::compare_copy(&a, &req(pa, pb), &ctx).is_err(),
            "con {pa:?}/{pb:?} no se puede comparar"
        );
    }
    assert!(!dir.path().join("v1 (comparación).pdf").exists());

    let report = compare::compare_copy(&a, &req(Some("clave-a"), Some("clave-b")), &ctx).unwrap();
    let svc = service();
    let id = svc.open_document(&report.outputs[0], None).unwrap();
    assert!(svc.document_info(id).unwrap().page_count >= 1);
    assert_paints(&svc, id);
    assert!(
        report.summary.contains("cambio") || report.summary.contains("palabras"),
        "{}",
        report.summary
    );
}

#[test]
fn insertar_paginas_de_otro_pdf_protegido() {
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let dest = encrypted(dir.path(), "organize/a.pdf", "dest", "clave-a");
    let src = encrypted(dir.path(), "organize/b.pdf", "origen", "clave-b");
    let svc = service();
    let nd = assert_still_protected(&svc, &dest, "clave-a");
    let ns = assert_still_protected(&svc, &src, "clave-b");

    let source = |pw: Option<&str>| InsertSource::File {
        path: src.clone(),
        password: pw.map(Into::into),
        pages: vec![],
    };
    assert!(matches!(
        organize::insert_copy(&dest, Some("clave-a"), &source(None), 0, "insertado"),
        Err(Error::Password)
    ));
    assert!(matches!(
        organize::insert_copy(&dest, None, &source(Some("clave-b")), 0, "insertado"),
        Err(Error::Password)
    ));
    let r = organize::insert_copy(&dest, Some("clave-a"), &source(Some("clave-b")), 0, "insertado").unwrap();
    // La copia conserva la protección del documento de destino.
    assert_eq!(assert_still_protected(&svc, &r.outputs[0], "clave-a"), nd + ns);
}

#[test]
fn validar_pdfa_de_un_pdf_protegido_usa_la_contrasena() {
    let dir = tempfile::tempdir().unwrap();
    let enc = encrypted(dir.path(), "organize/a.pdf", "a", "abre");
    let level = pliego_app_service::checks::Level::A2b;
    assert!(matches!(
        pliego_app_service::checks::pdfa(&enc, level, None),
        Err(Error::Password)
    ));
    let v = pliego_app_service::checks::pdfa(&enc, level, Some("abre")).unwrap();
    assert!(!v.issues.is_empty(), "un PDF cifrado no es PDF/A");
}

#[test]
fn estado_de_la_contrasena_de_los_archivos() {
    let dir = tempfile::tempdir().unwrap();
    let enc = encrypted(dir.path(), "organize/a.pdf", "a", "abre");
    assert_eq!(
        passwords::status(&enc, None).unwrap(),
        passwords::PasswordStatus::Required
    );
    assert_eq!(
        passwords::status(&enc, Some("mal")).unwrap(),
        passwords::PasswordStatus::Wrong
    );
    assert_eq!(
        passwords::status(&enc, Some("abre")).unwrap(),
        passwords::PasswordStatus::Open
    );
    assert_eq!(
        passwords::status(&corpus("organize/a.pdf"), None).unwrap(),
        passwords::PasswordStatus::Open
    );
}

/// OCR con la CLI real sobre un PDF protegido: la contraseña viaja por el entorno del proceso hijo (nunca por la línea
/// de órdenes), la copia sale con texto buscable y **sigue protegida**. Se salta sin CLI o sin el modelo `eng`
/// (en acr12 se ejecuta con `PLIEGO_CLI` apuntando a la CLI de la compilación).
#[test]
fn ocr_de_un_pdf_protegido_con_la_cli_real() {
    let _s = serial();
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
    if !ocr::languages(&cli)
        .map(|l| l.installed.iter().any(|x| x == "eng"))
        .unwrap_or(false)
    {
        eprintln!("sin el modelo eng: prueba de OCR omitida");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = encrypted(dir.path(), "basic/hola-ocr.pdf", "escaneo", "abre");
    let original = std::fs::read(&path).unwrap();
    let svc = service();
    let req = |password: Option<&str>| ocr::OcrRequest {
        path: path.clone(),
        langs: "eng".into(),
        force: true,
        password: password.map(Into::into),
        ..Default::default()
    };

    for bad in [None, Some("mala")] {
        let e = ocr::ocr_copy(&cli, &req(bad), &JobContext::silent()).unwrap_err();
        assert!(
            e.to_string().to_lowercase().contains("contraseña"),
            "el mensaje pide la contraseña: {e}"
        );
        assert!(!dir.path().join("escaneo (OCR).pdf").exists(), "sin copia a medias");
    }
    let report = ocr::ocr_copy(&cli, &req(Some("abre")), &JobContext::silent()).unwrap();
    let out = &report.outputs[0];
    assert_eq!(std::fs::read(&path).unwrap(), original, "el original no cambia");
    assert_still_protected(&svc, out, "abre");
    let rid = svc.open_document(out, Some("abre")).unwrap();
    assert!(
        !svc.page_text(rid, 0).unwrap().trim().is_empty(),
        "el PDF resultante tiene texto buscable"
    );
}
