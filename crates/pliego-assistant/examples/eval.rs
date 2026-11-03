//! Evaluación de preguntas con citas, sin MuPDF (sirve para probar el motor en equipos donde aún no compila).
//!
//! Uso: cargo run --release --features llama --example eval -- MODELO.gguf TEXTO.txt PREGUNTAS.tsv
//! - TEXTO.txt: páginas separadas por un salto de página (\f).
//! - PREGUNTAS.tsv: una por línea, `pregunta<TAB>página_esperada` (desde 1).

use std::path::PathBuf;

use pliego_assistant::{Assistant, DocumentText};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, text, questions] = args.as_slice() else {
        eprintln!("uso: eval MODELO.gguf TEXTO.txt PREGUNTAS.tsv");
        std::process::exit(2);
    };
    let doc = DocumentText::from_pages(
        std::fs::read_to_string(text)?
            .split('\x0c')
            .map(str::to_string)
            .collect(),
    );
    let assistant = Assistant::load(&PathBuf::from(model))?;
    let index = assistant.index(&doc, &pliego_core::JobContext::silent())?;
    let (mut ok, mut total) = (0, 0);
    for line in std::fs::read_to_string(questions)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let (q, expected) = line.split_once('\t').ok_or("falta la página esperada")?;
        let expected: u32 = expected.trim().parse()?;
        let started = std::time::Instant::now();
        let ans = assistant.ask_indexed(&index, q, &pliego_core::JobContext::silent())?;
        let pages: Vec<u32> = ans
            .citations
            .iter()
            .filter(|c| c.cited_by_model)
            .map(|c| c.page + 1)
            .collect();
        let hit = pages.contains(&expected);
        total += 1;
        ok += u32::from(hit);
        println!(
            "{} {q}\n   esperada p. {expected}, citadas {pages:?}, {:.1}s\n   {}\n",
            if hit { "✔" } else { "✘" },
            started.elapsed().as_secs_f32(),
            ans.text.replace('\n', " ")
        );
    }
    println!("Aciertos: {ok}/{total}");
    Ok(())
}
