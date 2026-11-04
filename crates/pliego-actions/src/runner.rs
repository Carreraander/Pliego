use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
use serde::Serialize;

use crate::action::{Action, ErrorPolicy};
use crate::inputs::Input;
use crate::journal::{self, Entry, Journal, Outcome};
use crate::registry::{Batch, Registry};

#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Carpeta donde se escriben los resultados (se crea si no existe). Nunca puede ser la de un original.
    pub output_dir: PathBuf,
    /// Retomar un lote anterior en `output_dir`: los archivos ya procesados (y sin cambios) se saltan.
    /// Solo vale si la acción no ha cambiado; si no, se empieza de cero con un aviso.
    pub resume: bool,
}

/// Un archivo que no se pudo procesar.
#[derive(Debug, Clone, Serialize)]
pub struct FileFailure {
    pub input: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RunReport {
    /// Archivos procesados en esta ejecución.
    pub processed: usize,
    /// Archivos saltados porque un lote anterior ya los había procesado.
    pub resumed: usize,
    pub failures: Vec<FileFailure>,
    pub report: Report,
}

fn describe(rel: &Path) -> String {
    rel.display().to_string()
}

/// Aplica `action` a cada PDF de `inputs`. Un archivo = todos los pasos encadenados; el resultado se escribe
/// en `opts.output_dir` conservando la estructura de carpetas. Tras cada archivo se guarda el diario, de modo
/// que si el lote se cancela (`Err(Cancelled)`) o se interrumpe se puede retomar con `resume`.
pub fn run(
    action: &Action,
    registry: &Registry,
    inputs: &[Input],
    opts: &RunOptions,
    ctx: &JobContext,
) -> Result<RunReport> {
    action.validate()?;
    for (i, step) in action.steps.iter().enumerate() {
        registry
            .get(&step.kind)?
            .validate(&step.options_value())
            .map_err(|e| Error::InvalidInput(format!("paso {} ({}): {e}", i + 1, step.kind)))?;
    }
    std::fs::create_dir_all(&opts.output_dir).map_err(|e| Error::io_at(&opts.output_dir, e))?;

    // Nunca escribir encima de un original.
    for input in inputs {
        let dest = opts.output_dir.join(&input.rel);
        if let (Ok(a), Ok(b)) = (input.path.canonicalize(), dest.canonicalize())
            && a == b
        {
            return Err(Error::InvalidInput(format!(
                "la salida de {} coincidiría con el original; elige otra carpeta de salida",
                input.path.display()
            )));
        }
    }

    let fingerprint = action.fingerprint();
    let mut report = RunReport::default();
    let mut journal = match Journal::load(&opts.output_dir) {
        Some(j) if opts.resume && j.action == fingerprint => j,
        Some(_) if opts.resume => {
            report
                .report
                .warnings
                .push("la acción ha cambiado desde el lote anterior: se empieza de cero".into());
            Journal::default()
        }
        _ => Journal::default(),
    };
    journal.action = fingerprint;
    if !opts.resume {
        journal.entries.clear();
    }

    let batch = Batch::default();
    let total = inputs.len() as u64;
    for (n, input) in inputs.iter().enumerate() {
        ctx.step(n as u64, total, &describe(&input.rel))?;
        let key = input.rel.to_string_lossy().replace('\\', "/");
        let (size, mtime) = journal::stamp_of(&input.path)?;
        if let Some((out, counters)) = journal.is_done(&key, size, mtime) {
            report.resumed += 1;
            report.report.outputs.push(out.to_path_buf());
            batch.restore(counters.clone());
            continue;
        }
        if journal.had_failed(&key)
            && inputs[n + 1..].iter().any(|later| {
                let k = later.rel.to_string_lossy().replace('\\', "/");
                journal::stamp_of(&later.path).is_ok_and(|(s, m)| journal.is_done(&k, s, m).is_some())
            })
        {
            report.report.warnings.push(format!(
                "{}: se reintenta tras archivos ya procesados; si el lote usa numeración continua puede repetir números (relanza sin --resume para renumerar todo)",
                describe(&input.rel)
            ));
        }
        let before = batch.snapshot();
        match process_one(action, registry, input, &opts.output_dir, &batch, ctx) {
            Ok((out, warnings)) => {
                report.processed += 1;
                report.report.outputs.push(out.clone());
                report
                    .report
                    .warnings
                    .extend(warnings.into_iter().map(|w| format!("{}: {w}", describe(&input.rel))));
                journal.entries.insert(
                    key,
                    Entry {
                        size,
                        mtime_secs: mtime,
                        counters: batch.snapshot(),
                        outcome: Outcome::Done { output: out },
                    },
                );
            }
            Err(Error::Cancelled) => {
                batch.restore(before);
                journal.save(&opts.output_dir)?;
                return Err(Error::Cancelled);
            }
            Err(e) => {
                batch.restore(before);
                let msg = e.to_string();
                report.report.warnings.push(format!("{}: {msg}", describe(&input.rel)));
                report.failures.push(FileFailure {
                    input: input.path.clone(),
                    error: msg.clone(),
                });
                journal.entries.insert(
                    key,
                    Entry {
                        size,
                        mtime_secs: mtime,
                        counters: Default::default(),
                        outcome: Outcome::Failed { error: msg },
                    },
                );
                if action.on_error == ErrorPolicy::Stop {
                    journal.save(&opts.output_dir)?;
                    break;
                }
            }
        }
        journal.save(&opts.output_dir)?;
    }
    ctx.step(total, total, "Terminado")?;
    journal.save(&opts.output_dir)?;

    report.report.summary = format!(
        "{} procesados, {} reanudados, {} con error",
        report.processed,
        report.resumed,
        report.failures.len()
    );
    Ok(report)
}

/// Encadena los pasos con temporales dentro de la carpeta de salida y deja el resultado final en su sitio.
fn process_one(
    action: &Action,
    registry: &Registry,
    input: &Input,
    out_dir: &Path,
    batch: &Batch,
    ctx: &JobContext,
) -> Result<(PathBuf, Vec<String>)> {
    let tmp = tempfile::Builder::new()
        .prefix(".pliego-run-")
        .tempdir_in(out_dir)
        .map_err(|e| Error::io_at(out_dir, e))?;
    let mut current = input.path.clone();
    let mut warnings = Vec::new();
    for (i, step) in action.steps.iter().enumerate() {
        if ctx.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let next = tmp.path().join(format!("paso{}.pdf", i + 1));
        let r = registry
            .get(&step.kind)?
            .run_in_batch(&current, &next, &step.options_value(), ctx, batch)
            .map_err(|e| match e {
                Error::Cancelled => e,
                other => Error::Other(format!("paso {} ({}): {other}", i + 1, step.kind)),
            })?;
        warnings.extend(r.warnings);
        if !next.exists() {
            return Err(Error::Other(format!(
                "el paso {} ({}) no produjo ningún archivo",
                i + 1,
                step.kind
            )));
        }
        current = next;
    }
    let dest = out_dir.join(&input.rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io_at(parent, e))?;
    }
    std::fs::rename(&current, &dest).map_err(|e| Error::io_at(&dest, e))?;
    Ok((dest, warnings))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use pliego_core::{CancelToken, NoProgress};
    use serde_json::Value;

    use super::*;
    use crate::action::Step;
    use crate::inputs::collect_inputs;
    use crate::registry::Batch;

    /// Paso de prueba: copia la entrada y añade una marca.
    fn mark(
        tag: &'static str,
        calls: &'static AtomicUsize,
    ) -> impl Fn(&Path, &Path, &Value, &JobContext) -> Result<Report> {
        move |i, o, _, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            let mut data = std::fs::read(i)?;
            data.extend_from_slice(tag.as_bytes());
            std::fs::write(o, data)?;
            Ok(Report::default())
        }
    }

    fn setup(n: usize) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("entrada");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        for i in 0..n {
            let p = if i % 2 == 0 { src.clone() } else { src.join("sub") };
            std::fs::write(p.join(format!("doc{i:03}.pdf")), format!("%PDF-{i}")).unwrap();
        }
        std::fs::write(src.join("nota.txt"), "no es pdf").unwrap();
        (dir, src)
    }

    fn action3() -> Action {
        Action {
            name: "t".into(),
            description: String::new(),
            on_error: ErrorPolicy::Continue,
            steps: vec![Step::new("a"), Step::new("b"), Step::new("c")],
        }
    }

    #[test]
    fn three_steps_over_100_files_keeps_structure() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let (dir, src) = setup(100);
        let mut reg = Registry::new();
        reg.register("a", mark("A", &CALLS))
            .register("b", mark("B", &CALLS))
            .register("c", mark("C", &CALLS));
        let out = dir.path().join("salida");
        let inputs = collect_inputs(&[src], Some(&out)).unwrap();
        assert_eq!(inputs.len(), 100);
        let r = run(
            &action3(),
            &reg,
            &inputs,
            &RunOptions {
                output_dir: out.clone(),
                resume: false,
            },
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(r.processed, 100);
        assert!(r.failures.is_empty());
        assert_eq!(CALLS.load(Ordering::SeqCst), 300);
        assert_eq!(std::fs::read(out.join("sub/doc001.pdf")).unwrap(), b"%PDF-1ABC");
        assert!(!out.join("nota.txt").exists());
        // Sin temporales sueltos.
        let leftovers: Vec<_> = std::fs::read_dir(&out)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".pliego-run-"))
            .collect();
        assert!(leftovers.is_empty());
    }

    struct CancelAfter {
        token: CancelToken,
        limit: usize,
        seen: AtomicUsize,
    }
    impl pliego_core::Progress for CancelAfter {
        fn report(&self, _d: u64, _t: u64, _m: &str) {
            if self.seen.fetch_add(1, Ordering::SeqCst) + 1 == self.limit {
                self.token.cancel();
            }
        }
    }

    #[test]
    fn cancel_then_resume_does_not_repeat_work() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let (dir, src) = setup(100);
        let mut reg = Registry::new();
        reg.register("a", mark("A", &CALLS))
            .register("b", mark("B", &CALLS))
            .register("c", mark("C", &CALLS));
        let out = dir.path().join("salida");
        let inputs = collect_inputs(&[src], Some(&out)).unwrap();
        let opts = |resume| RunOptions {
            output_dir: out.clone(),
            resume,
        };

        let token = CancelToken::new();
        let prog = CancelAfter {
            token: token.clone(),
            limit: 41,
            seen: AtomicUsize::new(0),
        };
        let ctx = JobContext::new(&prog, token);
        let err = run(&action3(), &reg, &inputs, &opts(false), &ctx).unwrap_err();
        assert!(matches!(err, Error::Cancelled));
        let done_first = CALLS.load(Ordering::SeqCst) / 3;
        assert!((38..=42).contains(&done_first), "{done_first}");

        let r = run(&action3(), &reg, &inputs, &opts(true), &JobContext::silent()).unwrap();
        assert_eq!(r.resumed, done_first);
        assert_eq!(r.processed, 100 - done_first);
        assert_eq!(
            CALLS.load(Ordering::SeqCst),
            300,
            "cada archivo se procesa una sola vez"
        );
        assert_eq!(r.report.outputs.len(), 100);
    }

    #[test]
    fn changed_action_discards_journal() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let (dir, src) = setup(4);
        let mut reg = Registry::new();
        reg.register("a", mark("A", &CALLS))
            .register("b", mark("B", &CALLS))
            .register("c", mark("C", &CALLS));
        let out = dir.path().join("salida");
        let inputs = collect_inputs(&[src], Some(&out)).unwrap();
        let o = RunOptions {
            output_dir: out,
            resume: true,
        };
        run(&action3(), &reg, &inputs, &o, &JobContext::silent()).unwrap();
        let mut other = action3();
        other.steps.pop();
        let r = run(&other, &reg, &inputs, &o, &JobContext::silent()).unwrap();
        assert_eq!(r.resumed, 0);
        assert_eq!(r.processed, 4);
        assert!(r.report.warnings.iter().any(|w| w.contains("ha cambiado")));
    }

    #[test]
    fn failures_are_reported_and_policy_stop_halts() {
        let (dir, src) = setup(6);
        let mut reg = Registry::new();
        reg.register("a", |i: &Path, o: &Path, _: &Value, _: &JobContext| -> Result<Report> {
            if i.to_string_lossy().contains("doc002") {
                return Err(Error::Password);
            }
            std::fs::copy(i, o)?;
            Ok(Report::default())
        });
        let mut a = Action {
            name: String::new(),
            description: String::new(),
            on_error: ErrorPolicy::Continue,
            steps: vec![Step::new("a")],
        };
        let inputs = collect_inputs(&[src], None).unwrap();
        let o = RunOptions {
            output_dir: dir.path().join("o1"),
            resume: false,
        };
        let r = run(&a, &reg, &inputs, &o, &JobContext::silent()).unwrap();
        assert_eq!((r.processed, r.failures.len()), (5, 1));
        assert!(r.failures[0].error.contains("contraseña"));

        a.on_error = ErrorPolicy::Stop;
        let o = RunOptions {
            output_dir: dir.path().join("o2"),
            resume: false,
        };
        let r = run(&a, &reg, &inputs, &o, &JobContext::silent()).unwrap();
        assert_eq!(r.failures.len(), 1);
        assert!(r.processed < 5);

        // Reanudar tras el fallo reintenta solo el que falló.
        let r2 = run(
            &a,
            &reg,
            &inputs,
            &RunOptions { resume: true, ..o },
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(r2.resumed, r.processed);
    }

    #[test]
    fn unknown_step_and_output_over_input_are_rejected() {
        let (dir, src) = setup(2);
        let reg = Registry::new();
        let inputs = collect_inputs(std::slice::from_ref(&src), None).unwrap();
        let o = RunOptions {
            output_dir: dir.path().join("o"),
            resume: false,
        };
        let e = run(&action3(), &reg, &inputs, &o, &NoProgressCtx::ctx()).unwrap_err();
        assert!(e.to_string().contains("paso desconocido"));

        let mut reg = Registry::new();
        reg.register("a", |_: &Path, _: &Path, _: &Value, _: &JobContext| -> Result<Report> {
            Ok(Report::default())
        });
        let a = Action {
            name: String::new(),
            description: String::new(),
            on_error: ErrorPolicy::Continue,
            steps: vec![Step::new("a")],
        };
        let o = RunOptions {
            output_dir: src,
            resume: false,
        };
        let e = run(&a, &reg, &inputs, &o, &NoProgressCtx::ctx()).unwrap_err();
        assert!(e.to_string().contains("coincidiría"));
    }

    /// Paso con contador de lote: escribe en la salida el número en el que empieza y avanza 2.
    struct Numbered;
    impl crate::registry::StepRunner for Numbered {
        fn run(&self, _: &Path, _: &Path, _: &Value, _: &JobContext) -> Result<Report> {
            unreachable!("solo se usa dentro de un lote")
        }
        fn run_in_batch(&self, i: &Path, o: &Path, _: &Value, _: &JobContext, batch: &Batch) -> Result<Report> {
            if i.to_string_lossy().contains("doc002") {
                return Err(Error::Password);
            }
            std::fs::write(o, batch.get("n").to_string())?;
            batch.add("n", 2);
            Ok(Report::default())
        }
    }

    fn starts(out: &Path) -> Vec<String> {
        fn walk(dir: &Path, acc: &mut Vec<(PathBuf, String)>) {
            for e in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, acc);
                } else if p.extension().is_some_and(|x| x == "pdf") {
                    acc.push((p.clone(), std::fs::read_to_string(&p).unwrap()));
                }
            }
        }
        let mut acc = Vec::new();
        walk(out, &mut acc);
        // El orden del lote es el de los nombres de archivo (doc000, doc001...), no el de carpetas.
        acc.sort_by_key(|(p, _)| p.file_name().map(|n| n.to_owned()));
        acc.into_iter().map(|(_, v)| v).collect()
    }

    #[test]
    fn batch_counters_continue_skip_failures_and_survive_resume() {
        let (dir, src) = setup(6);
        let mut reg = Registry::new();
        reg.register("n", Numbered);
        let a = Action {
            name: String::new(),
            description: String::new(),
            on_error: ErrorPolicy::Continue,
            steps: vec![Step::new("n")],
        };
        let inputs = collect_inputs(&[src], None).unwrap();
        let o = RunOptions {
            output_dir: dir.path().join("o"),
            resume: false,
        };
        let r = run(&a, &reg, &inputs, &o, &JobContext::silent()).unwrap();
        assert_eq!(r.failures.len(), 1);
        // Orden del lote: doc000, doc002 (falla, no consume números), doc004, luego la subcarpeta
        // (doc001, doc003, doc005): 0, -, 2, 4, 6, 8 sin huecos. Ordenados por nombre de archivo:
        let expected = ["0", "4", "6", "2", "8"];
        assert_eq!(starts(&o.output_dir), expected);

        // Reanudar: nada se repite y los contadores se retoman del diario; el que falló se reintenta
        // (sigue fallando) tras archivos ya numerados, y se avisa.
        let r2 = run(
            &a,
            &reg,
            &inputs,
            &RunOptions {
                resume: true,
                ..o.clone()
            },
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(r2.resumed, 5);
        assert!(
            r2.report.warnings.iter().any(|w| w.contains("numeración continua")),
            "{:?}",
            r2.report.warnings
        );
        assert_eq!(starts(&o.output_dir), expected);
    }

    struct NoProgressCtx;
    impl NoProgressCtx {
        fn ctx() -> JobContext<'static> {
            JobContext::new(&NoProgress, CancelToken::new())
        }
    }
}
