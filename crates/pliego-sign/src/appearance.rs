//! Apariencia de la firma visible: un recuadro con el nombre, la fecha y el motivo.

use lopdf::{Dictionary, Object, dictionary};

/// Texto PDF en WinAnsi (como Latin-1): los caracteres fuera de él se sustituyen por `?`.
fn win_ansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            if (c as u32) < 256 && c != '\r' && c != '\n' {
                c as u8
            } else {
                b'?'
            }
        })
        .collect()
}

fn escape(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

/// Divide `text` en líneas de como mucho `max` caracteres, cortando en espacios si puede.
fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        while word.chars().count() > max {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            let head: String = word.chars().take(max).collect();
            word = word.chars().skip(max).collect();
            lines.push(head);
        }
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > max {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(&word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Flujo de apariencia (XObject de formulario) de `w` × `h` puntos con las líneas de texto dadas.
/// Devuelve el diccionario del flujo y su contenido.
pub fn build(w: f32, h: f32, text_lines: &[String]) -> (Dictionary, Vec<u8>) {
    let size = 9.0f32.min((h - 6.0).max(4.0));
    let lead = size * 1.2;
    // Ancho medio de Helvetica ≈ 0,5 em.
    let max_chars = (((w - 6.0) / (size * 0.5)).floor() as usize).max(4);
    let max_lines = (((h - 6.0) / lead).floor() as usize).max(1);
    let lines: Vec<String> = text_lines
        .iter()
        .flat_map(|l| wrap(l, max_chars))
        .take(max_lines)
        .collect();

    let mut c = Vec::new();
    c.extend_from_slice(format!("0.4 w 0.15 0.25 0.55 RG 0.5 0.5 {:.2} {:.2} re S\n", w - 1.0, h - 1.0).as_bytes());
    c.extend_from_slice(
        format!(
            "BT /F1 {size:.2} Tf 0.1 0.1 0.2 rg {lead:.2} TL 3 {:.2} Td\n",
            h - 3.0 - size
        )
        .as_bytes(),
    );
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            c.extend_from_slice(b"T* ");
        }
        c.push(b'(');
        c.extend(escape(&win_ansi(l)));
        c.extend_from_slice(b") Tj\n");
    }
    c.extend_from_slice(b"ET\n");

    let font = dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
    };
    let dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => vec![0.into(), 0.into(), Object::Real(w), Object::Real(h)],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        "Length" => c.len() as i64,
    };
    (dict, c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_text_wraps_and_is_clipped() {
        let lines = wrap("Firmado digitalmente por Persona con un nombre largo", 20);
        assert!(lines.iter().all(|l| l.chars().count() <= 20), "{lines:?}");
        assert!(lines.len() >= 3);
    }

    #[test]
    fn accents_survive_and_other_chars_become_question_marks() {
        assert_eq!(win_ansi("Ñandú €"), vec![0xD1, b'a', b'n', b'd', 0xFA, b' ', b'?']);
    }

    #[test]
    fn parentheses_are_escaped() {
        let (_, content) = build(200.0, 60.0, &["a (b) c".into()]);
        assert!(String::from_utf8_lossy(&content).contains("a \\(b\\) c"));
    }
}
