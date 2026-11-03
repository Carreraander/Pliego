//! Normalización de texto para la recuperación (minúsculas, sin acentos, sin palabras vacías).

/// Quita acentos de las letras latinas habituales en español, catalán, gallego, portugués y francés.
fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        c => c,
    }
}

const STOPWORDS: &[&str] = &[
    // español
    "el", "la", "los", "las", "un", "una", "unos", "unas", "de", "del", "al", "y", "o", "u", "e", "en", "a", "que",
    "es", "por", "con", "para", "se", "su", "sus", "lo", "como", "mas", "pero", "sin", "sobre", "entre", "ser", "son",
    "este", "esta", "esto", "estos", "estas", "cual", "cuales", "cuando", "donde", "quien", "cuanto", "cuanta", "hay",
    "fue", "han", "ha", "le", "les", "me", "mi", "tu", "nos", "ya", "si", "no", // inglés
    "the", "an", "of", "and", "or", "to", "in", "on", "is", "are", "was", "were", "be", "by", "for", "with", "as", "at",
    "it", "its", "this", "that", "these", "those", "what", "which", "who", "when", "where", "how", "does", "do",
];

/// Convierte el texto en términos: minúsculas, sin acentos, sin palabras vacías y con un recorte muy ligero de
/// plurales (`contratos` → `contrato`, `plazos` → `plazo`).
pub fn terms(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        let w: String = word.chars().flat_map(char::to_lowercase).map(fold).collect();
        if STOPWORDS.contains(&w.as_str()) {
            continue;
        }
        out.push(stem(w));
    }
    out
}

fn stem(w: String) -> String {
    let n = w.chars().count();
    if n > 4 && w.ends_with("es") && !w.ends_with("ses") {
        return w[..w.len() - 2].to_string();
    }
    if n > 3 && w.ends_with('s') && !w.ends_with("ss") {
        return w[..w.len() - 1].to_string();
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_accents_and_drops_stopwords() {
        assert_eq!(
            terms("El plazo de ENTREGA será máximo"),
            ["plazo", "entrega", "sera", "maximo"]
        );
    }

    #[test]
    fn trims_plurals() {
        assert_eq!(terms("contratos pagos"), ["contrato", "pago"]);
        assert_eq!(terms("contrato"), ["contrato"]);
    }
}
