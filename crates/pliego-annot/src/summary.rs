use std::collections::BTreeMap;
use std::fmt::Write;

use mupdf::pdf::PdfDocument;
use pliego_core::Result;
use serde::{Deserialize, Serialize};

use crate::ops::list;
use crate::types::{AnnotInfo, AnnotKind, ReviewState};

/// Un comentario con sus respuestas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub comment: AnnotInfo,
    pub replies: Vec<AnnotInfo>,
}

/// Resumen estructurado de los comentarios de un documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentSummary {
    pub total: usize,
    pub by_kind: BTreeMap<String, usize>,
    pub by_author: BTreeMap<String, usize>,
    pub threads: Vec<Thread>,
}

fn kind_name(k: AnnotKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Agrupa las anotaciones en hilos (comentario + respuestas) en orden de página.
/// Las anotaciones sin contenido ni respuestas (por ejemplo, un simple resaltado) también cuentan.
pub fn summary(doc: &mut PdfDocument) -> Result<CommentSummary> {
    let all = list(doc, None)?;
    let mut by_kind = BTreeMap::new();
    let mut by_author = BTreeMap::new();
    for a in &all {
        *by_kind.entry(kind_name(a.kind)).or_insert(0) += 1;
        *by_author
            .entry(a.author.clone().unwrap_or_else(|| "(sin autor)".into()))
            .or_insert(0) += 1;
    }
    let mut threads: Vec<Thread> = Vec::new();
    for a in all.iter().filter(|a| a.reply_to.is_none()) {
        let replies = replies_of(&all, a.id.xref);
        threads.push(Thread {
            comment: a.clone(),
            replies,
        });
    }
    Ok(CommentSummary {
        total: all.len(),
        by_kind,
        by_author,
        threads,
    })
}

fn replies_of(all: &[AnnotInfo], parent: i32) -> Vec<AnnotInfo> {
    let mut out = Vec::new();
    for r in all.iter().filter(|r| r.reply_to == Some(parent)) {
        out.push(r.clone());
        out.extend(replies_of(all, r.id.xref));
    }
    out
}

fn state_name(s: ReviewState) -> &'static str {
    match s {
        ReviewState::Accepted => "aceptado",
        ReviewState::Rejected => "rechazado",
        ReviewState::Cancelled => "cancelado",
        ReviewState::Completed => "completado",
        ReviewState::None => "ninguno",
    }
}

impl CommentSummary {
    /// Versión en texto plano, legible por personas.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{} comentario(s)", self.total);
        for t in &self.threads {
            let c = &t.comment;
            let _ = writeln!(
                s,
                "- Pág. {} · {} · {}: {}",
                c.id.page + 1,
                kind_name(c.kind),
                c.author.as_deref().unwrap_or("(sin autor)"),
                c.contents.as_deref().unwrap_or("").replace('\n', " ")
            );
            for r in &t.replies {
                let st = r.state.map(|x| format!(" [{}]", state_name(x))).unwrap_or_default();
                let _ = writeln!(
                    s,
                    "    ↳ {}{}: {}",
                    r.author.as_deref().unwrap_or("(sin autor)"),
                    st,
                    r.contents.as_deref().unwrap_or("").replace('\n', " ")
                );
            }
        }
        s
    }
}
