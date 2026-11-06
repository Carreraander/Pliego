//! El trabajo de impresión, independiente del sistema: recorre las páginas elegidas, las coloca en la hoja y se las
//! da a un [`PrintTarget`]. En Windows el destino es el contexto de la impresora; en las pruebas, un registro.

use pliego_core::{JobContext, Result};

use crate::layout::{CopiesPlan, Placement, SheetMetrics, place};
use crate::options::PrintOptions;

/// Donde caen las páginas.
pub trait PrintTarget {
    /// Medidas de la hoja (con la orientación del trabajo).
    fn metrics(&self) -> SheetMetrics;
    fn start_document(&mut self, title: &str) -> Result<()>;
    /// Imprime la página `page` (desde 0) ya colocada: abre una hoja, la dibuja y la cierra.
    fn print_page(&mut self, page: u32, placement: &Placement) -> Result<()>;
    fn end_document(&mut self) -> Result<()>;
    /// Cancela el trabajo; no debe fallar.
    fn abort_document(&mut self);
}

/// Lo que se hizo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSummary {
    /// Hojas enviadas por Pliego (las copias que hace el controlador no cuentan).
    pub pages_sent: u32,
    /// Páginas distintas del documento.
    pub distinct_pages: u32,
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
    let pages = options.selection(sizes.len() as u32)?;
    let total = u64::from(pages.len() as u32) * u64::from(copies.set_repeats) * u64::from(copies.page_repeats);
    ctx.step(0, total, "Preparando la impresión")?;
    target.start_document(title)?;
    let sheet = target.metrics();
    let mut done = 0u64;
    let result: Result<()> = (|| {
        for _ in 0..copies.set_repeats {
            for &page in &pages {
                let (w, h) = sizes[page as usize];
                let placement = place(w, h, &sheet, options);
                for _ in 0..copies.page_repeats {
                    ctx.step(done, total, &format!("Imprimiendo la página {}", page + 1))?;
                    target.print_page(page, &placement)?;
                    done += 1;
                }
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            target.end_document()?;
            ctx.step(total, total, "Enviado a la impresora")?;
            Ok(JobSummary {
                pages_sent: done as u32,
                distinct_pages: pages.len() as u32,
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
        fn print_page(&mut self, page: u32, p: &Placement) -> Result<()> {
            if self.fail_on == Some(page) {
                return Err(Error::Other("la impresora no responde".into()));
            }
            self.log.push(format!("p{}:r{}", page + 1, p.rotate));
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
        assert_eq!((s.pages_sent, s.distinct_pages), (3, 3));
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
}
