//! Lectura de las citas `[p. 3]` que escribe el modelo.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

static CITE: LazyLock<Regex> = LazyLock::new(|| {
    // [p. 3] [p.3] [pág. 3] [págs. 3, 5-6] [p. 3; p. 7] [page 3]
    Regex::new(r"(?i)\[\s*(?:p\.|pp\.|pág\.|págs\.|pag\.|pags\.|page|pages)\s*([0-9][0-9\s,;\-–y]*)").unwrap()
});
static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([0-9]+)(?:\s*[-–]\s*([0-9]+))?").unwrap());

/// Páginas citadas en el texto, desde 1, ordenadas y sin repetir.
pub fn cited_pages(text: &str) -> Vec<u32> {
    let mut set = BTreeSet::new();
    for m in CITE.captures_iter(text) {
        for n in NUM.captures_iter(&m[1]) {
            let a: u32 = n[1].parse().unwrap_or(0);
            let b: u32 = n.get(2).and_then(|b| b.as_str().parse().ok()).unwrap_or(a);
            // Un rango absurdo no debe disparar un bucle enorme.
            if a > 0 && b >= a && b - a < 50 {
                set.extend(a..=b);
            }
        }
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_forms() {
        assert_eq!(cited_pages("Es de 6 meses [p. 12]."), [12]);
        assert_eq!(cited_pages("Ver [pág. 3] y [p.7]"), [3, 7]);
        assert_eq!(cited_pages("[págs. 3, 5-6]"), [3, 5, 6]);
        assert_eq!(cited_pages("[page 2][p. 2]"), [2]);
    }

    #[test]
    fn ignores_non_citations() {
        assert!(cited_pages("en la p. 4 del texto").is_empty());
        assert!(cited_pages("[p. 0]").is_empty());
        assert!(cited_pages("[p. 1-9999999]").is_empty());
    }
}
