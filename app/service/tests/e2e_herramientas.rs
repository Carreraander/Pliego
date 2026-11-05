//! Segunda pasada de punta a punta (matriz `docs/qa/validacion-funciones.md`): lo que genera cada herramienta se **abre
//! y se pinta en el worker**, como hace la app al terminar un trabajo y abrir el resultado. Cubre capas, XFDF, sellos,
//! firma digital y flujo de firmas, sanear, PDF/A, orden de tabulación, marcas de agua, encabezados, fondos, Bates,
//! medir y acciones en lote. Cada prueba parte de un PDF **abierto en el worker** (Windows lo bloqueaba antes).
//!
//! Va en su propio ejecutable por lo mismo que `e2e_documento_abierto.rs`: el worker hereda los archivos abiertos.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
use pliego_app_service::annot::{AnnotSpec, Editor};
use pliego_app_service::sign::{self, KeySource};
use pliego_app_service::{DocId, DocumentService};
use pliego_core::{JobContext, TileRequest};
use pliego_sign::SignOptions;
use pliego_signflow::SignerSpec;
use pliego_worker::WorkerConfig;
use rcgen::{CertificateParams, DnType, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256};

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

fn tile(svc: &DocumentService, id: DocId) -> Vec<u8> {
    let png = svc
        .tile(
            id,
            TileRequest {
                page: 0,
                zoom: 0.5, // la página A4 entera cabe en una tesela
                col: 0,
                row: 0,
                size: 512,
            },
        )
        .unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
    png
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

/// El resultado de un trabajo se abre en el visor y se pinta: devuelve el número de páginas.
fn assert_opens(svc: &DocumentService, path: &Path) -> u32 {
    let id = svc
        .open_document(path, None)
        .unwrap_or_else(|e| panic!("{} no se abre: {e}", path.display()));
    tile(svc, id);
    svc.document_info(id).unwrap().page_count
}

/// PFX autofirmado de pruebas (nunca claves reales).
fn pfx(dir: &Path, name: &str) -> KeySource {
    let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.distinguished_name.push(DnType::CommonName, name);
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::ContentCommitment];
    let cert = params.self_signed(&key).unwrap();
    let chain = PrivateKeyChain::new(
        b"k".to_vec(),
        PrivateKey::from_der(&key.serialize_der()).unwrap(),
        [Certificate::from_der(cert.der()).unwrap()],
    );
    let mut store = KeyStore::new();
    store.add_entry("firma", KeyStoreEntry::PrivateKeyChain(chain));
    let path = dir.join(format!("{name}.p12"));
    std::fs::write(&path, store.writer("prueba").write().unwrap()).unwrap();
    KeySource::Pfx {
        path,
        password: "prueba".into(),
    }
}

/// PDF de una página con un cuadrado negro dentro de la capa «Fondo» (grupo de contenido opcional).
fn layered_pdf() -> Vec<u8> {
    let content = "/OC /L0 BDC 0 g 0 0 100 100 re f EMC";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R] /D << /ON [5 0 R] /Order [5 0 R] >> >> >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << /Properties << /L0 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /OCG /Name (Fondo) >>".to_string(),
    ];
    let mut out = b"%PDF-1.5\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[test]
fn capas_se_ocultan_y_se_muestran_en_el_documento_abierto() {
    let _s = serial();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("capas.pdf");
    std::fs::write(&path, layered_pdf()).unwrap();
    let svc = service();
    let id = svc.open_document(&path, None).unwrap();

    let layers = svc.layers(id).unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, "Fondo");
    assert!(layers[0].visible);
    let with_layer = tile(&svc, id);

    let rev = svc.revision(id).unwrap();
    svc.set_layer_visible(id, layers[0].id, false).unwrap();
    assert!(svc.revision(id).unwrap() > rev, "ocultar una capa repinta");
    assert!(!svc.layers(id).unwrap()[0].visible);
    assert_ne!(tile(&svc, id), with_layer, "sin la capa el cuadrado negro desaparece");

    svc.set_layer_visible(id, layers[0].id, true).unwrap();
    assert_eq!(tile(&svc, id), with_layer, "al mostrarla vuelve igual");
    assert!(svc.set_layer_visible(id, 9999, false).is_err(), "capa inexistente");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        layered_pdf(),
        "las capas no tocan el archivo"
    );
}

#[test]
fn xfdf_exporta_e_importa_los_comentarios() {
    let _s = serial();
    use pliego_app_service::annot;
    let (dir, path, svc, id) = open("basic/hola.pdf");
    let original = std::fs::read(&path).unwrap();

    let hits = svc.search(id, "Hola", 5).unwrap();
    let mut spec = AnnotSpec::new(
        hits[0].page as usize,
        pliego_annot::Geometry::Highlight {
            quads: hits[0].quads.clone(),
        },
    );
    spec.contents = Some("revisar".into());
    let bytes = Editor::default().add(&path, None, &spec, "copia de seguridad").unwrap();
    svc.reload_bytes(id, &bytes).unwrap();

    let exported = annot::export_xfdf_copy(&path, None, "comentarios").unwrap();
    assert!(exported.outputs[0].extension().is_some_and(|e| e == "xfdf"));
    assert!(
        std::fs::read_to_string(&exported.outputs[0])
            .unwrap()
            .contains("highlight")
    );

    // Se importa en un PDF limpio (el original antes de comentar) y sale una copia con el comentario.
    let clean = dir.path().join("limpio.pdf");
    std::fs::write(&clean, &original).unwrap();
    let imported = annot::import_xfdf_copy(&clean, None, &exported.outputs[0], "con comentarios").unwrap();
    assert_eq!(annot::summary_of(&imported.outputs[0], None).unwrap().total, 1);
    assert_opens(&svc, &imported.outputs[0]);
    assert_eq!(std::fs::read(&clean).unwrap(), original, "el PDF limpio no cambia");
}

#[test]
fn sellos_en_una_copia_se_abren_y_se_pintan() {
    let _s = serial();
    use pliego_app_service::stamps::{Place, StampRequest, stamp_annot_copy};
    let (_dir, path, svc, id) = open("organize/a.pdf");
    let before = tile(&svc, id);
    let original = std::fs::read(&path).unwrap();
    let n = svc.document_info(id).unwrap().page_count;

    for source in [
        pliego_annot::StampSource::Preset {
            preset: pliego_annot::StampPreset::Aprobado,
        },
        pliego_annot::StampSource::Text {
            text: "REVISADO".into(),
            color: [0.8, 0.1, 0.1],
        },
    ] {
        let report = stamp_annot_copy(
            &path,
            "all",
            None,
            &StampRequest {
                source,
                place: Place::BottomRight,
            },
            &JobContext::silent(),
        )
        .unwrap();
        let out = &report.outputs[0];
        assert_eq!(assert_opens(&svc, out), n);
        let oid = svc.open_document(out, None).unwrap();
        assert_ne!(tile(&svc, oid), before, "el sello se ve en la hoja");
    }
    assert!(
        stamp_annot_copy(
            &path,
            "all",
            None,
            &StampRequest {
                source: pliego_annot::StampSource::Text {
                    text: "  ".into(),
                    color: [0.0; 3]
                },
                place: Place::Center
            },
            &JobContext::silent()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn firmar_validar_y_flujo_de_firmas_desde_un_pdf_abierto() {
    let _s = serial();
    let (dir, path, svc, _id) = open("basic/hola.pdf");
    let original = std::fs::read(&path).unwrap();
    let ctx = JobContext::silent();

    let signed = dir.path().join("hola (firmado).pdf");
    sign::sign_pdf(
        &path,
        &signed,
        &pfx(dir.path(), "Ana Prueba"),
        SignOptions {
            reason: Some("Aprobado".into()),
            ..Default::default()
        },
        &ctx,
    )
    .unwrap();
    assert_opens(&svc, &signed);
    let sigs = sign::validate(&signed).unwrap();
    assert_eq!(sigs.len(), 1);
    assert!(sigs[0].integrity);
    assert_eq!(sigs[0].signer, "Ana Prueba");

    // Una firma no puede reutilizar el archivo de entrada como salida.
    assert!(sign::sign_pdf(&path, &path, &pfx(dir.path(), "Otra"), SignOptions::default(), &ctx).is_err());

    // Flujo: dos firmantes en orden y registro de auditoría; cada paso se abre y se pinta.
    let prepared = dir.path().join("flujo.pdf");
    sign::flow_prepare(
        &path,
        &prepared,
        vec![SignerSpec::new("Ana"), SignerSpec::new("Beto")],
        &ctx,
    )
    .unwrap();
    assert_opens(&svc, &prepared);
    assert_eq!(sign::flow_next(&prepared).unwrap().unwrap().spec.name, "Ana");
    let one = dir.path().join("flujo1.pdf");
    sign::flow_sign(&prepared, &one, &pfx(dir.path(), "Ana"), SignOptions::default(), &ctx).unwrap();
    assert_opens(&svc, &one);
    let two = dir.path().join("flujo2.pdf");
    let r = sign::flow_sign(&one, &two, &pfx(dir.path(), "Beto"), SignOptions::default(), &ctx).unwrap();
    assert!(r.summary.contains("todos"));
    assert_opens(&svc, &two);
    assert!(sign::flow_next(&two).unwrap().is_none());
    assert_eq!(sign::flow_audit(&two).unwrap().len(), 3);
    assert_eq!(sign::validate(&two).unwrap().len(), 2);

    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn sanear_pdfa_y_orden_de_tabulacion_dan_copias_que_se_abren() {
    let _s = serial();
    use pliego_app_service::{checks, convert, protect};
    let (_dir, path, svc, _id) = open("protect/sucio.pdf");
    let original = std::fs::read(&path).unwrap();
    let n = svc
        .document_info(svc.open_document(&path, None).unwrap())
        .unwrap()
        .page_count;
    let ctx = JobContext::silent();

    let req: protect::SanitizeRequest = serde_json::from_value(serde_json::json!({
        "path": path,
        "options": serde_json::to_value(pliego_protect::SanitizeOptions::default()).unwrap()
    }))
    .unwrap();
    let clean = protect::sanitize(&req, &ctx).unwrap();
    assert_eq!(assert_opens(&svc, &clean.outputs[0]), n);

    let pdfa = convert::pdfa_copy(&path, "2b", "PDF-A", &ctx).unwrap();
    assert_eq!(assert_opens(&svc, &pdfa.outputs[0]), n);
    assert!(convert::pdfa_copy(&path, "9z", "PDF-A", &ctx).is_err());

    let tabbed = checks::tab_order_copy(&path, None, "orden").unwrap();
    assert_eq!(assert_opens(&svc, &tabbed.outputs[0]), n);

    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn marcas_encabezados_fondos_y_bates_se_pintan_en_la_hoja() {
    let _s = serial();
    use pliego_app_service::stamp::{StampOp, stamp_copy};
    let (_dir, path, svc, id) = open("organize/a.pdf");
    let before = tile(&svc, id);
    let n = svc.document_info(id).unwrap().page_count;
    let ctx = JobContext::silent();

    let ops = [
        (
            r#"{"op":"watermark","content":{"kind":"text","text":"COPIA","style":{}},"opacity":0.3,"rotation":30,"behind":false}"#,
            Some("COPIA"),
        ),
        (
            r#"{"op":"header_footer","header":{"left":"","center":"ENCABEZADO","right":""},"footer":{"left":"","center":"{page}/{pages}","right":""},"style":{"font":"Helvetica","size":10,"color":[0,0,0]},"margin":36}"#,
            Some("ENCABEZADO"),
        ),
        (
            r#"{"op":"background","fill":{"kind":"color","rgb":[1,0.9,0.6]},"opacity":1}"#,
            None,
        ),
        (
            r#"{"op":"bates","prefix":"CASO-","digits":6,"start":1}"#,
            Some("CASO-000001"),
        ),
    ];
    for (json, text) in ops {
        let op: StampOp = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        let report = stamp_copy(&path, "all", None, &op, &ctx).unwrap();
        let out = &report.outputs[0];
        assert_eq!(assert_opens(&svc, out), n);
        let oid = svc.open_document(out, None).unwrap();
        assert_ne!(tile(&svc, oid), before, "{json} cambia lo que se ve");
        if let Some(t) = text {
            assert!(svc.page_text(oid, 0).unwrap().contains(t), "{t} en el texto de la hoja");
        }
    }
}

#[test]
fn medir_guarda_la_escala_y_la_medida_en_el_pdf_abierto() {
    let _s = serial();
    use pliego_app_service::measure;
    let (_dir, path, svc, id) = open("basic/hola.pdf");
    let ed = Editor::default();

    assert!(
        measure::page_scale(&path, None, 0).unwrap().is_none(),
        "sin escala al principio"
    );
    let scale = measure::ratio_scale(100.0, pliego_measure::Unit::M).unwrap();
    let before = svc.revision(id).unwrap();
    let (bytes, m) = measure::save(
        &ed,
        &path,
        None,
        "copia de seguridad",
        0,
        &scale,
        &[[100.0, 100.0], [200.0, 100.0]],
        false,
    )
    .unwrap();
    svc.reload_bytes(id, &bytes).unwrap();
    assert!(svc.revision(id).unwrap() > before);
    tile(&svc, id);
    assert!(m.length > 0.0, "{m:?}");
    assert!(
        measure::page_scale(&path, None, 0).unwrap().is_some(),
        "la escala queda en la página"
    );
    // Deshacer devuelve el archivo anterior.
    let undone = ed.undo(&path).unwrap().unwrap();
    svc.reload_bytes(id, &undone).unwrap();
    assert!(measure::page_scale(&path, None, 0).unwrap().is_none());
}

#[test]
fn acciones_en_lote_sobre_una_carpeta_con_un_pdf_abierto() {
    let _s = serial();
    use pliego_app_service::actions::{BatchRequest, run_batch};
    let (dir, path, svc, _id) = open("organize/a.pdf");
    let original = std::fs::read(&path).unwrap();
    let n = assert_opens(&svc, &path);
    let action = dir.path().join("accion.toml");
    std::fs::write(&action, "[[steps]]\nkind = \"pages.delete\"\npages = \"1\"\n").unwrap();
    let report = run_batch(
        &BatchRequest {
            action,
            inputs: vec![path.clone()],
            suffix: "lote".into(),
            ..Default::default()
        },
        &JobContext::silent(),
    )
    .unwrap();
    assert!(!report.outputs.is_empty(), "{report:?}");
    for out in &report.outputs {
        assert_eq!(assert_opens(&svc, out), n - 1);
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn trabajos_sobre_un_pdf_protegido_con_la_contrasena_recordada() {
    let _s = serial();
    use pliego_app_service::{checks, optimize, organize, protect, stamp};
    let (dir, path, svc, _id) = open("organize/a.pdf");
    let ctx = JobContext::silent();
    let req: protect::ProtectRequest = serde_json::from_value(serde_json::json!({
        "path": path, "userPassword": "abre", "ownerPassword": "manda"
    }))
    .unwrap();
    let enc = protect::protect(&req, &ctx).unwrap().outputs.remove(0);
    let copy = dir.path().join("cifrado.pdf");
    std::fs::copy(&enc, &copy).unwrap();

    // El visor pide la contraseña y, con ella, abre; los trabajos reciben esa misma contraseña.
    assert!(matches!(
        svc.open_document(&copy, None),
        Err(pliego_core::Error::Password)
    ));
    let id = svc.open_document(&copy, Some("abre")).unwrap();
    let n = svc.document_info(id).unwrap().page_count;
    let pw = Some("abre");

    assert!(
        checks::preflight(&copy, "basic", None).is_err(),
        "sin contraseña no se puede leer"
    );
    assert!(checks::preflight(&copy, "basic", pw).is_ok());
    assert!(checks::accessibility(&copy, pw).is_ok());
    assert!(optimize::analyze(&copy, pw).is_ok());

    let compressed = optimize::compress_copy(
        &copy,
        &optimize::CompressRequest {
            preset: "ebook".into(),
            suffix: "comprimido".into(),
            password: pw.map(Into::into),
            ..Default::default()
        },
        &ctx,
    )
    .unwrap();
    assert!(compressed.outputs[0].exists());

    let numbered = organize::number_copy(
        &copy,
        pw,
        &organize::NumberRequest {
            position: organize::NumberPosition::FooterCenter,
            format: "{page}/{pages}".into(),
            from_page: 0,
            size: 10.0,
            also_labels: false,
        },
        "numerado",
        &ctx,
    )
    .unwrap();
    assert!(numbered.outputs[0].exists());

    let marked = stamp::stamp_copy(&copy, "all", pw, &stamp::StampOp::Bates(Default::default()), &ctx).unwrap();
    assert!(marked.outputs[0].exists());
    // Cada copia conserva el número de páginas y **sigue protegida** con la misma contraseña.
    for out in [&compressed.outputs[0], &numbered.outputs[0], &marked.outputs[0]] {
        assert!(
            matches!(svc.open_document(out, None), Err(pliego_core::Error::Password)),
            "{} sigue protegida",
            out.display()
        );
        let oid = svc.open_document(out, Some("abre")).unwrap();
        assert_eq!(svc.document_info(oid).unwrap().page_count, n);
    }
}
