use pliego_core::{EngineDocument, JobContext, Result};
use serde::{Deserialize, Serialize};

/// Texto de un documento, página a página (índice 0 = primera página).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DocumentText {
    pub pages: Vec<String>,
}

/// Fragmento de una página, con su página de origen (desde 0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chunk {
    pub page: u32,
    pub text: String,
}

/// Tamaño objetivo de un fragmento, en caracteres (≈ 120 tokens).
const TARGET_CHARS: usize = 600;

impl DocumentText {
    pub fn from_pages(pages: Vec<String>) -> Self {
        Self { pages }
    }

    /// Extrae el texto de todo el documento con el motor PDF.
    pub fn from_engine<D: EngineDocument>(doc: &D, ctx: &JobContext) -> Result<Self> {
        let total = doc.page_count()?;
        let mut pages = Vec::with_capacity(total as usize);
        for p in 0..total {
            ctx.step(u64::from(p), u64::from(total), "Leyendo el texto del documento")?;
            pages.push(doc.page_text(p)?);
        }
        Ok(Self { pages })
    }

    /// `true` si no hay texto utilizable (p. ej. un escaneado sin OCR).
    pub fn is_empty(&self) -> bool {
        self.pages.iter().all(|p| p.trim().is_empty())
    }

    /// Trocea cada página en párrafos de ≈ 600 caracteres. Un fragmento nunca cruza de página, así la cita
    /// es siempre exacta.
    pub fn chunks(&self) -> Vec<Chunk> {
        let mut out = Vec::new();
        for (i, page) in self.pages.iter().enumerate() {
            let mut cur = String::new();
            for para in page.split("\n\n").flat_map(split_long) {
                let para = para.trim();
                if para.is_empty() {
                    continue;
                }
                if !cur.is_empty() && cur.chars().count() + para.chars().count() > TARGET_CHARS {
                    out.push(Chunk {
                        page: i as u32,
                        text: std::mem::take(&mut cur),
                    });
                }
                if !cur.is_empty() {
                    cur.push('\n');
                }
                cur.push_str(para);
            }
            if !cur.trim().is_empty() {
                out.push(Chunk {
                    page: i as u32,
                    text: cur,
                });
            }
        }
        out
    }
}

/// Parte un párrafo demasiado largo por frases o, en último caso, por tamaño.
fn split_long(para: &str) -> Vec<String> {
    if para.chars().count() <= TARGET_CHARS {
        return vec![para.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in para.split_whitespace() {
        if cur.chars().count() + word.chars().count() + 1 > TARGET_CHARS && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_keep_their_page() {
        let doc = DocumentText::from_pages(vec!["Uno\n\nDos".into(), "".into(), "Tres".into()]);
        let c = doc.chunks();
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].page, c[0].text.as_str()), (0, "Uno\nDos"));
        assert_eq!(c[1].page, 2);
    }

    #[test]
    fn long_paragraphs_are_split() {
        let doc = DocumentText::from_pages(vec!["palabra ".repeat(400)]);
        let c = doc.chunks();
        assert!(c.len() >= 4);
        assert!(c.iter().all(|c| c.text.chars().count() <= TARGET_CHARS + 20));
    }

    #[test]
    fn scanned_documents_are_empty() {
        assert!(DocumentText::from_pages(vec!["  ".into(), "\n".into()]).is_empty());
    }
}
