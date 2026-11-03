use std::sync::Arc;

use pliego_core::{Error, JobContext, Result};
use serde::{Deserialize, Serialize};

use crate::backend::{GenerateRequest, LlmBackend, Message};
use crate::chunk::DocumentText;
use crate::citations::cited_pages;
use crate::config::AssistantConfig;
use crate::retrieve::{DocIndex, Passage};

/// Cita de una respuesta: página (desde 0) y el fragmento que la respalda.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Citation {
    pub page: u32,
    pub snippet: String,
    /// `true` si el propio modelo citó esta página; `false` si se añade como mejor fuente recuperada.
    pub cited_by_model: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Answer {
    pub text: String,
    pub citations: Vec<Citation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub text: String,
    /// Páginas con texto que se han leído.
    pub pages_read: u32,
}

pub struct Assistant {
    backend: Arc<dyn LlmBackend>,
    config: AssistantConfig,
}

const NOT_FOUND: &str = "No he encontrado información sobre eso en el documento.";

const ASK_SYSTEM: &str = "Eres el asistente de un lector de PDF y trabajas sin conexión. Responde a la pregunta \
usando SOLO los fragmentos del documento que se te dan. Cada fragmento empieza con su página, por ejemplo [p. 3]. \
Cita las páginas en las que te basas con ese mismo formato, [p. N], justo después de cada afirmación. Si los \
fragmentos no contienen la respuesta, dilo claramente y no inventes nada. Responde en el idioma de la pregunta, \
de forma breve y precisa.";

const SUMMARY_SYSTEM: &str = "Eres el asistente de un lector de PDF y trabajas sin conexión. Resume el texto que se \
te da en el idioma del propio texto. Conserva cifras, fechas y nombres importantes. Indica las páginas de origen \
con el formato [p. N]. No añadas información que no esté en el texto.";

impl Assistant {
    /// Crea el asistente sobre un motor ya cargado.
    pub fn with_backend(backend: Arc<dyn LlmBackend>, config: AssistantConfig) -> Self {
        Self { backend, config }
    }

    /// Carga un modelo GGUF del disco. Disponible con la característica `llama`.
    pub fn load(_model: &std::path::Path) -> Result<Self> {
        Err(Error::Unsupported(
            "esta compilación no incluye el motor llama.cpp (característica `llama`)".into(),
        ))
    }

    pub fn config(&self) -> &AssistantConfig {
        &self.config
    }

    /// Índice del documento (léxico y, si el modelo lo permite, vectores). Se puede reutilizar entre preguntas.
    pub fn index(&self, doc: &DocumentText, ctx: &JobContext) -> Result<DocIndex> {
        DocIndex::build(doc, self.backend.as_ref(), ctx)
    }

    /// Responde una pregunta sobre el documento citando páginas.
    pub fn ask(&self, doc: &DocumentText, question: &str) -> Result<Answer> {
        let ctx = JobContext::silent();
        let index = self.index(doc, &ctx)?;
        self.ask_indexed(&index, question, &ctx)
    }

    pub fn ask_indexed(&self, index: &DocIndex, question: &str, ctx: &JobContext) -> Result<Answer> {
        let question = question.trim();
        if question.is_empty() {
            return Err(Error::InvalidInput("la pregunta está vacía".into()));
        }
        if index.is_empty() {
            return Err(Error::InvalidInput(
                "el documento no tiene texto; si es un escaneado, aplica OCR primero".into(),
            ));
        }
        let qvec = if index.has_vectors() {
            self.backend.embed(question)?
        } else {
            None
        };
        let passages = index.search(question, qvec.as_deref(), self.config.top_k);
        if passages.is_empty() {
            return Ok(Answer {
                text: NOT_FOUND.into(),
                citations: vec![],
            });
        }

        // Los fragmentos se recortan para que quepan en la ventana del modelo.
        let budget = self.char_budget(1000);
        let mut context = String::new();
        let mut used: Vec<&Passage> = Vec::new();
        for p in &passages {
            let block = format!("[p. {}]\n{}\n\n", p.page + 1, p.text);
            if !used.is_empty() && context.chars().count() + block.chars().count() > budget {
                break;
            }
            context.push_str(&block);
            used.push(p);
        }

        let req = GenerateRequest {
            messages: vec![
                Message::system(ASK_SYSTEM),
                Message::user(format!("Fragmentos del documento:\n\n{context}Pregunta: {question}")),
            ],
            max_tokens: self.config.max_answer_tokens,
            temperature: 0.1,
        };
        let text = self.run(&req, ctx)?;
        let citations = build_citations(&text, &used);
        Ok(Answer { text, citations })
    }

    /// Resume el documento entero. Si no cabe en la ventana del modelo, resume por tramos y luego junta.
    pub fn summarize(&self, doc: &DocumentText, ctx: &JobContext) -> Result<Summary> {
        if doc.is_empty() {
            return Err(Error::InvalidInput(
                "el documento no tiene texto; si es un escaneado, aplica OCR primero".into(),
            ));
        }
        let budget = self.char_budget(800);
        let mut pieces: Vec<(String, bool)> = Vec::new(); // (texto, ya es un resumen parcial)
        let pages_read = doc.pages.iter().filter(|p| !p.trim().is_empty()).count() as u32;
        for (i, page) in doc.pages.iter().enumerate() {
            if page.trim().is_empty() {
                continue;
            }
            let body: String = page.trim().chars().take(budget).collect();
            pieces.push((format!("[p. {}]\n{}", i + 1, body), false));
        }

        // Se agrupa en tramos que caben y se resume cada uno; se repite hasta quedar uno solo.
        let mut level = 0;
        loop {
            let groups = group(&pieces, budget);
            let last = groups.len() == 1;
            let mut next = Vec::with_capacity(groups.len());
            for (n, g) in groups.iter().enumerate() {
                ctx.step(n as u64, groups.len() as u64, "Resumiendo el documento")?;
                let instruction = if last {
                    "Escribe un resumen del documento en un párrafo corto seguido de 3 a 6 puntos clave."
                } else {
                    "Resume este tramo en un párrafo corto."
                };
                let req = GenerateRequest {
                    messages: vec![
                        Message::system(SUMMARY_SYSTEM),
                        Message::user(format!("{instruction}\n\n{g}")),
                    ],
                    max_tokens: self.config.max_answer_tokens,
                    temperature: 0.2,
                };
                next.push((self.run(&req, ctx)?, true));
            }
            if last {
                return Ok(Summary {
                    text: next.remove(0).0,
                    pages_read,
                });
            }
            level += 1;
            if level > 6 {
                return Err(Error::Other(
                    "el documento es demasiado largo para resumirlo con este modelo".into(),
                ));
            }
            pieces = next;
        }
    }

    /// Texto alternativo para una imagen PNG. Requiere un modelo multimodal.
    pub fn suggest_alt_text(&self, png: &[u8]) -> Result<String> {
        self.backend.describe_image(
            png,
            "Describe esta imagen en una frase breve y objetiva, adecuada como texto alternativo para accesibilidad.",
        )
    }

    /// Caracteres disponibles para el texto de entrada (≈ 3 por token), dejando sitio a la respuesta.
    fn char_budget(&self, reserve_tokens: usize) -> usize {
        let tokens = self
            .backend
            .context_tokens()
            .saturating_sub(self.config.max_answer_tokens as usize + reserve_tokens);
        (tokens * 3).max(600)
    }

    fn run(&self, req: &GenerateRequest, ctx: &JobContext) -> Result<String> {
        let mut cancelled = false;
        let text = self.backend.generate(req, &mut |_| {
            cancelled = ctx.cancel.is_cancelled();
            !cancelled
        })?;
        if cancelled {
            return Err(Error::Cancelled);
        }
        Ok(text.trim().to_string())
    }
}

fn group(pieces: &[(String, bool)], budget: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for (p, _) in pieces {
        if !cur.is_empty() && cur.chars().count() + p.chars().count() + 2 > budget {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str("\n\n");
        }
        cur.push_str(p);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Citas válidas: las páginas que el modelo citó y que estaban entre los fragmentos entregados. Si no citó
/// ninguna válida, se ofrecen las mejores fuentes recuperadas (marcadas como no citadas por el modelo).
fn build_citations(answer: &str, used: &[&Passage]) -> Vec<Citation> {
    let mut out: Vec<Citation> = Vec::new();
    for page1 in cited_pages(answer) {
        let page = page1 - 1;
        if let Some(p) = used.iter().find(|p| p.page == page) {
            out.push(Citation {
                page,
                snippet: snippet(&p.text),
                cited_by_model: true,
            });
        }
    }
    if out.is_empty() {
        for p in used.iter().take(3) {
            if !out.iter().any(|c| c.page == p.page) {
                out.push(Citation {
                    page: p.page,
                    snippet: snippet(&p.text),
                    cited_by_model: false,
                });
            }
        }
    }
    out
}

fn snippet(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= 200 {
        one_line
    } else {
        let cut: String = one_line.chars().take(200).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Motor de pruebas: devuelve una respuesta fija y guarda las peticiones.
    struct Fake {
        reply: String,
        ctx: usize,
        seen: Mutex<Vec<GenerateRequest>>,
    }

    impl Fake {
        fn new(reply: &str, ctx: usize) -> Arc<Self> {
            Arc::new(Self {
                reply: reply.into(),
                ctx,
                seen: Mutex::new(vec![]),
            })
        }
    }

    impl LlmBackend for Fake {
        fn generate(&self, req: &GenerateRequest, on_token: &mut dyn FnMut(&str) -> bool) -> Result<String> {
            self.seen.lock().unwrap().push(req.clone());
            on_token(&self.reply);
            Ok(self.reply.clone())
        }
        fn context_tokens(&self) -> usize {
            self.ctx
        }
    }

    fn doc() -> DocumentText {
        DocumentText::from_pages(vec![
            "Introducción general al proyecto de reforma.".into(),
            "El plazo de entrega de las obras será de seis meses desde la firma.".into(),
            "La penalización por retraso es de 500 euros por día.".into(),
        ])
    }

    #[test]
    fn answer_cites_the_page_the_model_used() {
        let fake = Fake::new("Seis meses [p. 2].", 8192);
        let a = Assistant::with_backend(fake.clone(), AssistantConfig::default());
        let ans = a.ask(&doc(), "¿Cuál es el plazo de entrega?").unwrap();
        assert_eq!(ans.citations.len(), 1);
        assert_eq!(ans.citations[0].page, 1);
        assert!(ans.citations[0].cited_by_model);
        let prompt = &fake.seen.lock().unwrap()[0].messages[1].content;
        assert!(prompt.contains("[p. 2]") && prompt.contains("seis meses"));
    }

    #[test]
    fn invented_pages_are_discarded_and_sources_offered() {
        let fake = Fake::new("Seis meses [p. 99].", 8192);
        let a = Assistant::with_backend(fake, AssistantConfig::default());
        let ans = a.ask(&doc(), "plazo de entrega").unwrap();
        assert_eq!(ans.citations[0].page, 1);
        assert!(!ans.citations[0].cited_by_model);
    }

    #[test]
    fn no_relevant_text_does_not_call_the_model() {
        let fake = Fake::new("inventado", 8192);
        let a = Assistant::with_backend(fake.clone(), AssistantConfig::default());
        let ans = a.ask(&doc(), "¿Qué dice sobre xilófonos?").unwrap();
        assert_eq!(ans.text, NOT_FOUND);
        assert!(fake.seen.lock().unwrap().is_empty());
    }

    #[test]
    fn scanned_document_asks_for_ocr() {
        let a = Assistant::with_backend(Fake::new("x", 8192), AssistantConfig::default());
        let e = a.ask(&DocumentText::from_pages(vec!["".into()]), "hola").unwrap_err();
        assert!(e.to_string().contains("OCR"));
    }

    #[test]
    fn summary_of_a_long_document_is_map_reduce() {
        let fake = Fake::new("resumen [p. 1]", 2000);
        let a = Assistant::with_backend(fake.clone(), AssistantConfig::default());
        let long = DocumentText::from_pages(
            (0..20)
                .map(|i| format!("Página {i}. {}", "texto ".repeat(300)))
                .collect(),
        );
        let s = a.summarize(&long, &JobContext::silent()).unwrap();
        assert_eq!(s.pages_read, 20);
        assert_eq!(s.text, "resumen [p. 1]");
        assert!(
            fake.seen.lock().unwrap().len() > 2,
            "debe haber tramos y una unión final"
        );
    }

    #[test]
    fn short_summary_is_a_single_call() {
        let fake = Fake::new("ok", 8192);
        let a = Assistant::with_backend(fake.clone(), AssistantConfig::default());
        a.summarize(&doc(), &JobContext::silent()).unwrap();
        assert_eq!(fake.seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn cancellation_is_honoured() {
        let a = Assistant::with_backend(Fake::new("x", 8192), AssistantConfig::default());
        let ctx = JobContext::silent();
        ctx.cancel.cancel();
        assert!(matches!(a.summarize(&doc(), &ctx), Err(Error::Cancelled)));
    }

    #[test]
    fn alt_text_needs_a_multimodal_model() {
        let a = Assistant::with_backend(Fake::new("x", 8192), AssistantConfig::default());
        assert!(matches!(a.suggest_alt_text(&[]), Err(Error::Unsupported(_))));
    }

    #[test]
    fn disabled_by_default() {
        assert!(!AssistantConfig::default().enabled);
    }
}
