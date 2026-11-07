//! El trabajo de impresión, independiente del sistema: recorre las páginas elegidas, las coloca en la hoja y se las
//! da a un [`PrintTarget`]. En Windows el destino es el contexto de la impresora; en las pruebas, un registro.

use pliego_core::{JobContext, Result};

use crate::layout::{CopiesPlan, SheetMetrics, SheetPlan, plan_sheets};
use crate::options::PrintOptions;

/// Donde caen las páginas.
pub trait PrintTarget {
    /// Medidas de la hoja (con la orientación del trabajo).
    fn metrics(&self) -> SheetMetrics;
    fn start_document(&mut self, title: &str) -> Result<()>;
    /// Imprime una cara: abre la hoja, dibuja en ella cada página ya colocada (y su marco si se pide) y la cierra.
    /// Sin páginas es una hoja en blanco.
    fn print_sheet(&mut self, sheet: &SheetPlan) -> Result<()>;
    fn end_document(&mut self) -> Result<()>;
    /// Cancela el trabajo; no debe fallar.
    fn abort_document(&mut self);
}

/// Lo que se hizo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSummary {
    /// Caras enviadas por Pliego (las copias que hace el controlador no cuentan).
    pub sheets_sent: u32,
    /// Caras distintas del trabajo.
    pub distinct_sheets: u32,
}

/// Imprime las páginas de `options` (con `sizes[i]` = tamaño en puntos de la página `i`).
pub fn run_job(
    sizes: &[(f32, f32)],
    options: &PrintOptions,
    copies: CopiesPlan,
    target: &mut dyn PrintTarget,
    title: &str,
    ctx: &JobContext,
) -> Result<JobSummary> {
    // Se calcula todo antes de tocar la impresora: una selección vacía o unas opciones no válidas no abren trabajo.
    let sheet = target.metrics();
    let plans = plan_sheets(sizes, &sheet, options)?;
    let blank = SheetPlan::default();
    let padding = u64::from(copies.pad_odd_sets && plans.len() % 2 == 1) * u64::from(copies.set_repeats - 1);
    let total = plans.len() as u64 * u64::from(copies.set_repeats) * u64::from(copies.page_repeats) + padding;
    ctx.step(0, total, "Preparando la impresión")?;
    target.start_document(title)?;
    let mut done = 0u64;
    let result: Result<()> = (|| {
        for set in 0..copies.set_repeats {
            for (n, plan) in plans.iter().enumerate() {
                let message = match plan.slots.as_slice() {
                    [one] => format!("Imprimiendo la página {}", one.page + 1),
                    _ => format!("Imprimiendo la hoja {}", n + 1),
                };
                for _ in 0..copies.page_repeats {
                    ctx.step(done, total, &message)?;
                    target.print_sheet(plan)?;
                    done += 1;
                }
            }
            if copies.pad_odd_sets && plans.len() % 2 == 1 && set + 1 < copies.set_repeats {
                ctx.step(done, total, "Añadiendo una hoja en blanco")?;
                target.print_sheet(&blank)?;
                done += 1;
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            target.end_document()?;
            ctx.step(total, total, "Enviado a la impresora")?;
            Ok(JobSummary {
                sheets_sent: done as u32,
                distinct_sheets: plans.len() as u32,
            })
        }
        Err(e) => {
            target.abort_document();
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use pliego_core::{CancelToken, Progress};

    use super::*;
    use crate::layout::plan_copies;
    use crate::options::Subset;
    use pliego_core::Error;

    #[derive(Default)]
    struct Fake {
        log: Vec<String>,
        fail_on: Option<u32>,
    }

    impl PrintTarget for Fake {
        fn metrics(&self) -> SheetMetrics {
            SheetMetrics::plain(595.0, 842.0)
        }
        fn start_document(&mut self, title: &str) -> Result<()> {
            self.log.push(format!("doc:{title}"));
            Ok(())
        }
        fn print_sheet(&mut self, sheet: &SheetPlan) -> Result<()> {
            if sheet.slots.iter().any(|s| self.fail_on == Some(s.page)) {
                return Err(Error::Other("la impresora no responde".into()));
            }
            if sheet.slots.is_empty() {
                self.log.push("blank".into());
            } else {
                let parts: Vec<String> = sheet
                    .slots
                    .iter()
                    .map(|s| format!("p{}:r{}", s.page + 1, s.placement.rotate))
                    .collect();
                self.log.push(parts.join("+"));
            }
            Ok(())
        }
        fn end_document(&mut self) -> Result<()> {
            self.log.push("end".into());
            Ok(())
        }
        fn abort_document(&mut self) {
            self.log.push("abort".into());
        }
    }

    const SIZES: [(f32, f32); 3] = [(595.0, 842.0), (842.0, 595.0), (595.0, 842.0)];

    #[test]
    fn prints_selected_pages_in_order_with_rotation() {
        let mut t = Fake::default();
        let opts = PrintOptions {
            subset: Subset::All,
            ..PrintOptions::default()
        };
        let s = run_job(
            &SIZES,
            &opts,
            plan_copies(1, true, 1, false),
            &mut t,
            "doc",
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(t.log, ["doc:doc", "p1:r0", "p2:r1", "p3:r0", "end"]);
        assert_eq!((s.sheets_sent, s.distinct_sheets), (3, 3));
    }

    #[test]
    fn software_copies_collated_and_uncollated() {
        let opts = PrintOptions {
            pages: "1-2".into(),
            ..PrintOptions::default()
        };
        let mut t = Fake::default();
        run_job(
            &SIZES,
            &opts,
            plan_copies(2, true, 1, false),
            &mut t,
            "d",
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(t.log, ["doc:d", "p1:r0", "p2:r1", "p1:r0", "p2:r1", "end"]);
        let mut t = Fake::default();
        run_job(
            &SIZES,
            &opts,
            plan_copies(2, false, 1, false),
            &mut t,
            "d",
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(t.log, ["doc:d", "p1:r0", "p1:r0", "p2:r1", "p2:r1", "end"]);
    }

    #[test]
    fn failure_aborts_the_document() {
        let mut t = Fake {
            fail_on: Some(1),
            ..Fake::default()
        };
        let err = run_job(
            &SIZES,
            &PrintOptions::default(),
            plan_copies(1, true, 1, false),
            &mut t,
            "d",
            &JobContext::silent(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("no responde"));
        assert_eq!(t.log.last().unwrap(), "abort");
        assert!(!t.log.contains(&"end".to_string()));
    }

    struct CancelAfter(AtomicU32, CancelToken, u32);
    impl Progress for CancelAfter {
        fn report(&self, _done: u64, _total: u64, _m: &str) {
            if self.0.fetch_add(1, Ordering::SeqCst) + 1 >= self.2 {
                self.1.cancel();
            }
        }
    }

    #[test]
    fn cancelling_stops_and_aborts() {
        let token = CancelToken::new();
        let progress = CancelAfter(AtomicU32::new(0), token.clone(), 2);
        let ctx = JobContext::new(&progress, token);
        let mut t = Fake::default();
        let err = run_job(
            &SIZES,
            &PrintOptions::default(),
            plan_copies(1, true, 1, false),
            &mut t,
            "d",
            &ctx,
        )
        .unwrap_err();
        assert!(matches!(err, Error::Cancelled));
        assert_eq!(t.log.last().unwrap(), "abort");
        // Solo se llegó a imprimir la primera página.
        assert_eq!(t.log.iter().filter(|l| l.starts_with('p')).count(), 1);
    }

    #[test]
    fn empty_selection_is_an_error_before_touching_the_printer() {
        let mut t = Fake::default();
        let opts = PrintOptions {
            pages: "9".into(),
            ..PrintOptions::default()
        };
        assert!(
            run_job(
                &SIZES,
                &opts,
                plan_copies(1, true, 1, false),
                &mut t,
                "d",
                &JobContext::silent()
            )
            .is_err()
        );
        assert!(t.log.is_empty());
    }

    #[test]
    fn booklet_job_prints_two_pages_per_side() {
        let mut t = Fake::default();
        let opts = PrintOptions {
            booklet: crate::options::Booklet::Both,
            ..PrintOptions::default()
        };
        let sizes = [(595.0, 842.0); 4];
        let s = run_job(
            &sizes,
            &opts,
            plan_copies(1, true, 1, false),
            &mut t,
            "d",
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(t.log, ["doc:d", "p4:r0+p1:r0", "p2:r0+p3:r0", "end"]);
        assert_eq!(s.sheets_sent, 2);
    }

    #[test]
    fn odd_duplex_sets_get_a_blank_side_between_copies() {
        let mut t = Fake::default();
        let sizes = [(595.0, 842.0); 3];
        let copies = crate::layout::plan_copies_for(2, true, 1, false, true);
        run_job(
            &sizes,
            &PrintOptions::default(),
            copies,
            &mut t,
            "d",
            &JobContext::silent(),
        )
        .unwrap();
        assert_eq!(
            t.log,
            [
                "doc:d", "p1:r0", "p2:r0", "p3:r0", "blank", "p1:r0", "p2:r0", "p3:r0", "end"
            ]
        );
    }
}
