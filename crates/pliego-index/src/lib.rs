//! Índice de texto de carpetas de PDFs con Tantivy (MIT). Un documento del índice = una página.
//!
//! - [`Index::add_folder`] indexa de forma incremental: solo se vuelve a leer lo que cambió (tamaño o fecha) y
//!   se olvida lo que ya no existe en la carpeta.
//! - [`Index::search`] devuelve archivo, página (desde 0), puntuación y un fragmento con las coincidencias.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pliego_core::{Error, JobContext, Report, Result};
use pliego_pdf::{Document, TextFlags};
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::{QueryParser, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, STORED, STRING, Schema, TEXT, Value};
use tantivy::snippet::SnippetGenerator;
use tantivy::{IndexWriter, ReloadPolicy, TantivyDocument, Term, doc};

fn idx_err(e: impl std::fmt::Display) -> Error {
    Error::Other(format!("error del índice: {e}"))
}

/// Una coincidencia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub path: PathBuf,
    /// Página desde 0.
    pub page: u32,
    pub score: f32,
    /// Fragmento de texto de la página.
    pub snippet: String,
    /// Rangos `[inicio, fin)` en bytes de `snippet` que coinciden con la consulta.
    pub highlights: Vec<(usize, usize)>,
}

struct Fields {
    path: Field,
    page: Field,
    text: Field,
    size: Field,
    mtime: Field,
}

pub struct Index {
    index: tantivy::Index,
    f: Fields,
}

fn schema() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let f = Fields {
        path: b.add_text_field("path", STRING | STORED),
        page: b.add_u64_field("page", STORED),
        text: b.add_text_field("text", TEXT | STORED),
        size: b.add_u64_field("size", STORED),
        mtime: b.add_u64_field("mtime", STORED),
    };
    (b.build(), f)
}

fn is_pdf(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn list_pdfs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| Error::io_at(dir, e))?
        .collect::<std::io::Result<_>>()
        .map_err(|e| Error::io_at(dir, e))?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let ty = e.file_type().map_err(|e| Error::io_at(&p, e))?;
        if ty.is_dir() {
            list_pdfs(&p, out)?;
        } else if ty.is_file() && is_pdf(&p) {
            out.push(p);
        }
    }
    Ok(())
}

fn stamp(p: &Path) -> Result<(u64, u64)> {
    let md = std::fs::metadata(p).map_err(|e| Error::io_at(p, e))?;
    let secs = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Ok((md.len(), secs))
}

/// Texto de cada página. `Err(Password)` si el documento está cifrado.
fn extract_pages(path: &Path) -> Result<Vec<String>> {
    let bytes = std::fs::read(path).map_err(|e| Error::io_at(path, e))?;
    let doc = Document::from_bytes(&bytes, "application/pdf")?;
    if doc.needs_password()? {
        return Err(Error::Password);
    }
    let n = doc.page_count()?;
    let mut pages = Vec::with_capacity(n.max(0) as usize);
    for i in 0..n {
        let page = doc.load_page(i)?;
        let text = page.to_text_page(TextFlags::empty())?.to_text()?;
        pages.push(text);
    }
    Ok(pages)
}

impl Index {
    /// Abre el índice de `dir`, creándolo si no existe.
    pub fn open(dir: &Path) -> Result<Index> {
        std::fs::create_dir_all(dir).map_err(|e| Error::io_at(dir, e))?;
        let (schema, f) = schema();
        let mmap = tantivy::directory::MmapDirectory::open(dir).map_err(idx_err)?;
        let index = tantivy::Index::open_or_create(mmap, schema).map_err(idx_err)?;
        Ok(Index { index, f })
    }

    fn reader(&self) -> Result<tantivy::IndexReader> {
        self.index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(idx_err)
    }

    /// Tamaño y fecha con los que se indexó `path`, si está en el índice.
    fn indexed_stamp(&self, searcher: &tantivy::Searcher, path: &str) -> Result<Option<(u64, u64)>> {
        let q = TermQuery::new(Term::from_field_text(self.f.path, path), IndexRecordOption::Basic);
        let top = searcher
            .search(&q, &TopDocs::with_limit(1).order_by_score())
            .map_err(idx_err)?;
        let Some((_, addr)) = top.first() else {
            return Ok(None);
        };
        let d: TantivyDocument = searcher.doc(*addr).map_err(idx_err)?;
        let get = |f| d.get_first(f).and_then(|v| v.as_u64()).unwrap_or(0);
        Ok(Some((get(self.f.size), get(self.f.mtime))))
    }

    /// Indexa (o actualiza) todos los PDFs de `folder`, recursivo. Los archivos cifrados o dañados se saltan con
    /// un aviso. Los que ya no existen en la carpeta salen del índice.
    pub fn add_folder(&self, folder: &Path, ctx: &JobContext) -> Result<Report> {
        let mut files = Vec::new();
        list_pdfs(folder, &mut files)?;
        let root = folder.canonicalize().map_err(|e| Error::io_at(folder, e))?;
        let mut writer: IndexWriter = self.index.writer(50_000_000).map_err(idx_err)?;
        let searcher = self.reader()?.searcher();
        let mut report = Report::default();
        let (mut added, mut unchanged) = (0, 0);
        let mut present = HashSet::new();
        let total = files.len() as u64;
        let (mut t_extract, mut t_write) = (std::time::Duration::ZERO, std::time::Duration::ZERO);

        for (n, file) in files.iter().enumerate() {
            ctx.step(n as u64, total, &file.display().to_string())?;
            let canon = file.canonicalize().map_err(|e| Error::io_at(file, e))?;
            let key = canon.to_string_lossy().into_owned();
            present.insert(key.clone());
            let (size, mtime) = stamp(file)?;
            if self.indexed_stamp(&searcher, &key)? == Some((size, mtime)) {
                unchanged += 1;
                continue;
            }
            let t0 = std::time::Instant::now();
            let pages = extract_pages(file);
            t_extract += t0.elapsed();
            let pages = match pages {
                Ok(p) => p,
                Err(e) => {
                    report.warnings.push(format!("{}: {e}", file.display()));
                    continue;
                }
            };
            let t0 = std::time::Instant::now();
            writer.delete_term(Term::from_field_text(self.f.path, &key));
            for (i, text) in pages.into_iter().enumerate() {
                writer
                    .add_document(doc!(
                        self.f.path => key.as_str(),
                        self.f.page => i as u64,
                        self.f.text => text,
                        self.f.size => size,
                        self.f.mtime => mtime,
                    ))
                    .map_err(idx_err)?;
            }
            t_write += t0.elapsed();
            added += 1;
        }

        // Olvidar lo que ya no está en la carpeta.
        let mut removed = 0;
        let prefix = root.to_string_lossy().into_owned();
        for seg in searcher.segment_readers() {
            let store = seg.get_store_reader(1).map_err(idx_err)?;
            let alive = seg.alive_bitset();
            for doc_id in 0..seg.max_doc() {
                if alive.is_some_and(|b| !b.is_alive(doc_id)) {
                    continue;
                }
                let d: TantivyDocument = store.get(doc_id).map_err(idx_err)?;
                let Some(p) = d.get_first(self.f.path).and_then(|v| v.as_str()) else {
                    continue;
                };
                let page = d.get_first(self.f.page).and_then(|v| v.as_u64()).unwrap_or(0);
                if page == 0 && p.starts_with(&prefix) && !present.contains(p) {
                    writer.delete_term(Term::from_field_text(self.f.path, p));
                    removed += 1;
                }
            }
        }
        let t0 = std::time::Instant::now();
        writer.commit().map_err(idx_err)?;
        t_write += t0.elapsed();
        report.summary = format!(
            "{added} indexados, {unchanged} sin cambios, {removed} olvidados (extracción {:.2} s, escritura {:.2} s)",
            t_extract.as_secs_f64(),
            t_write.as_secs_f64()
        );
        Ok(report)
    }

    /// Busca `query` (sintaxis de Tantivy: palabras, `"frase exacta"`, `+obligatoria`, `-excluida`).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>> {
        let searcher = self.reader()?.searcher();
        let parser = QueryParser::for_index(&self.index, vec![self.f.text]);
        let q = parser
            .parse_query(query)
            .map_err(|e| Error::InvalidInput(format!("consulta no válida: {e}")))?;
        let top = searcher
            .search(&q, &TopDocs::with_limit(limit.max(1)).order_by_score())
            .map_err(idx_err)?;
        let snippets = SnippetGenerator::create(&searcher, &q, self.f.text).map_err(idx_err)?;
        let mut hits = Vec::with_capacity(top.len());
        for (score, addr) in top {
            let d: TantivyDocument = searcher.doc(addr).map_err(idx_err)?;
            let path = d.get_first(self.f.path).and_then(|v| v.as_str()).unwrap_or_default();
            let page = d.get_first(self.f.page).and_then(|v| v.as_u64()).unwrap_or(0);
            let s = snippets.snippet_from_doc(&d);
            hits.push(Hit {
                path: PathBuf::from(path),
                page: page as u32,
                score,
                snippet: s.fragment().to_string(),
                highlights: s.highlighted().iter().map(|r| (r.start, r.end)).collect(),
            });
        }
        Ok(hits)
    }
}
