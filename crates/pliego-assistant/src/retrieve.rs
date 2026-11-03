//! Recuperación de pasajes: BM25 léxico y, si el modelo ofrece vectores, fusión con similitud coseno.

use std::collections::HashMap;

use pliego_core::{JobContext, Result};
use serde::{Deserialize, Serialize};

use crate::backend::LlmBackend;
use crate::chunk::{Chunk, DocumentText};
use crate::text::terms;

/// Pasaje devuelto por la búsqueda.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Passage {
    pub page: u32,
    pub text: String,
    pub score: f32,
}

/// Índice de un documento: se construye una vez y sirve para varias preguntas.
pub struct DocIndex {
    chunks: Vec<Chunk>,
    tf: Vec<HashMap<String, u32>>,
    len: Vec<f32>,
    df: HashMap<String, u32>,
    avg_len: f32,
    vectors: Option<Vec<Vec<f32>>>,
}

impl DocIndex {
    /// Índice solo léxico (no necesita modelo).
    pub fn lexical(doc: &DocumentText) -> Self {
        let chunks = doc.chunks();
        let mut tf = Vec::with_capacity(chunks.len());
        let mut len = Vec::with_capacity(chunks.len());
        let mut df: HashMap<String, u32> = HashMap::new();
        for c in &chunks {
            let mut m: HashMap<String, u32> = HashMap::new();
            let ts = terms(&c.text);
            len.push(ts.len() as f32);
            for t in ts {
                *m.entry(t).or_default() += 1;
            }
            for t in m.keys() {
                *df.entry(t.clone()).or_default() += 1;
            }
            tf.push(m);
        }
        let avg_len = if len.is_empty() {
            0.0
        } else {
            len.iter().sum::<f32>() / len.len() as f32
        };
        Self {
            chunks,
            tf,
            len,
            df,
            avg_len,
            vectors: None,
        }
    }

    /// Índice léxico más vectores del modelo, si este los calcula; si no, queda solo léxico.
    pub fn build(doc: &DocumentText, backend: &dyn LlmBackend, ctx: &JobContext) -> Result<Self> {
        let mut idx = Self::lexical(doc);
        let total = idx.chunks.len() as u64;
        let mut vectors = Vec::with_capacity(idx.chunks.len());
        for (i, c) in idx.chunks.iter().enumerate() {
            ctx.step(i as u64, total, "Indexando el documento")?;
            match backend.embed(&c.text)? {
                Some(v) => vectors.push(v),
                None => return Ok(idx),
            }
        }
        idx.vectors = Some(vectors);
        Ok(idx)
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn has_vectors(&self) -> bool {
        self.vectors.is_some()
    }

    fn bm25(&self, query: &str) -> Vec<f32> {
        const K1: f32 = 1.4;
        const B: f32 = 0.75;
        let n = self.chunks.len() as f32;
        let q = terms(query);
        (0..self.chunks.len())
            .map(|i| {
                let mut s = 0.0;
                for t in &q {
                    let Some(&f) = self.tf[i].get(t) else { continue };
                    let df = *self.df.get(t).unwrap_or(&0) as f32;
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    let f = f as f32;
                    s += idf * f * (K1 + 1.0) / (f + K1 * (1.0 - B + B * self.len[i] / self.avg_len.max(1.0)));
                }
                s
            })
            .collect()
    }

    /// Los `k` pasajes más relevantes para la pregunta, de mejor a peor. `query_vec` es el vector de la
    /// pregunta si el modelo lo calcula.
    pub fn search(&self, query: &str, query_vec: Option<&[f32]>, k: usize) -> Vec<Passage> {
        let lex = self.bm25(query);
        let mut scores: Vec<f32> = lex.clone();
        if let (Some(vecs), Some(q)) = (&self.vectors, query_vec) {
            // Fusión por rango recíproco (RRF) entre léxico y vectores.
            let dense: Vec<f32> = vecs.iter().map(|v| cosine(v, q)).collect();
            let (rl, rd) = (ranks(&lex), ranks(&dense));
            scores = (0..lex.len())
                .map(|i| {
                    let l = if lex[i] > 0.0 { 1.0 / (60.0 + rl[i] as f32) } else { 0.0 };
                    l + 1.0 / (60.0 + rd[i] as f32)
                })
                .collect();
        }
        let mut order: Vec<usize> = (0..scores.len()).filter(|&i| scores[i] > 0.0).collect();
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
        order
            .into_iter()
            .take(k)
            .map(|i| Passage {
                page: self.chunks[i].page,
                text: self.chunks[i].text.clone(),
                score: scores[i],
            })
            .collect()
    }
}

fn ranks(scores: &[f32]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    let mut r = vec![0; scores.len()];
    for (rank, i) in order.into_iter().enumerate() {
        r[i] = rank;
    }
    r
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut na, mut nb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> DocumentText {
        DocumentText::from_pages(vec![
            "Introducción general al proyecto de reforma.".into(),
            "El plazo de entrega de las obras será de seis meses desde la firma.".into(),
            "La penalización por retraso es de 500 euros por día.".into(),
        ])
    }

    #[test]
    fn finds_the_right_page() {
        let idx = DocIndex::lexical(&doc());
        let r = idx.search("¿Cuál es el plazo de entrega?", None, 2);
        assert_eq!(r[0].page, 1);
        let r = idx.search("penalizaciones por retrasos", None, 1);
        assert_eq!(r[0].page, 2);
    }

    #[test]
    fn nothing_relevant_gives_nothing() {
        let idx = DocIndex::lexical(&doc());
        assert!(idx.search("xylophone", None, 3).is_empty());
    }

    #[test]
    fn dense_fusion_can_rescue_a_paraphrase() {
        let mut idx = DocIndex::lexical(&doc());
        idx.vectors = Some(vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![0.7, 0.7]]);
        // Ninguna palabra coincide, pero el vector de la pregunta apunta a la página 2 (índice 1).
        let r = idx.search("¿cuándo termina el trabajo?", Some(&[0.0, 1.0]), 1);
        assert_eq!(r[0].page, 1);
    }
}
