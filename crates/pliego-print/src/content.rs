//! Lector mínimo de flujos de contenido PDF: separa operadores y operandos.

/// Operando de un operador de contenido. Las cadenas se descartan; las matrices se aplanan.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Arg {
    Num(f32),
    Name(String),
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Op {
    pub name: String,
    pub args: Vec<Arg>,
}

impl Op {
    pub fn num(&self, i: usize) -> Option<f32> {
        match self.args.get(i) {
            Some(Arg::Num(n)) => Some(*n),
            _ => None,
        }
    }
    pub fn name_arg(&self, i: usize) -> Option<&str> {
        match self.args.get(i) {
            Some(Arg::Name(n)) => Some(n),
            _ => None,
        }
    }
    pub fn nums(&self) -> Vec<f32> {
        self.args
            .iter()
            .filter_map(|a| if let Arg::Num(n) = a { Some(*n) } else { None })
            .collect()
    }
}

fn is_space(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}
fn is_delim(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

pub(crate) fn parse(data: &[u8]) -> Vec<Op> {
    let mut ops = Vec::new();
    let mut args: Vec<Arg> = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if is_space(b) {
            i += 1;
        } else if b == b'%' {
            while i < data.len() && data[i] != b'\n' && data[i] != b'\r' {
                i += 1;
            }
        } else if b == b'(' {
            let mut depth = 0;
            while i < data.len() {
                match data[i] {
                    b'\\' => i += 1,
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            i += 1;
            args.push(Arg::Other);
        } else if b == b'<' {
            if data.get(i + 1) == Some(&b'<') {
                i += 2;
            } else {
                while i < data.len() && data[i] != b'>' {
                    i += 1;
                }
                i += 1;
                args.push(Arg::Other);
            }
        } else if matches!(b, b'>' | b'[' | b']' | b'{' | b'}' | b')') {
            i += 1;
        } else if b == b'/' {
            let start = i + 1;
            i = start;
            while i < data.len() && !is_space(data[i]) && !is_delim(data[i]) {
                i += 1;
            }
            args.push(Arg::Name(decode_name(&data[start..i])));
        } else {
            let start = i;
            while i < data.len() && !is_space(data[i]) && !is_delim(data[i]) {
                i += 1;
            }
            let word = &data[start..i];
            if let Some(n) = std::str::from_utf8(word).ok().and_then(parse_number) {
                args.push(Arg::Num(n));
            } else {
                let name = String::from_utf8_lossy(word).into_owned();
                if name == "BI" {
                    i = skip_inline_image(data, i);
                }
                ops.push(Op {
                    name,
                    args: std::mem::take(&mut args),
                });
            }
        }
    }
    ops
}

fn parse_number(s: &str) -> Option<f32> {
    if s.bytes().all(|c| c.is_ascii_digit() || matches!(c, b'+' | b'-' | b'.')) && s.bytes().any(|c| c.is_ascii_digit())
    {
        s.parse().ok()
    } else {
        None
    }
}

fn decode_name(raw: &[u8]) -> String {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'#'
            && i + 2 < raw.len()
            && let Some(v) = std::str::from_utf8(&raw[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(raw[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Salta los datos de una imagen en línea (`BI ... ID <datos> EI`).
fn skip_inline_image(data: &[u8], mut i: usize) -> usize {
    while i + 2 < data.len() {
        if data[i] == b'E'
            && data[i + 1] == b'I'
            && (i == 0 || is_space(data[i - 1]))
            && (i + 2 >= data.len() || is_space(data[i + 2]))
        {
            return i + 2;
        }
        i += 1;
    }
    data.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_operators() {
        let ops = parse(b"q 1 0 0 1 10 20 cm 0.5 w /F1 12 Tf (a w b) Tj [ (x) -20 (y) ] TJ 0 0 1 0 k S Q");
        let names: Vec<_> = ops.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["q", "cm", "w", "Tf", "Tj", "TJ", "k", "S", "Q"]);
        assert_eq!(ops[1].nums(), [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
        assert_eq!(ops[3].name_arg(0), Some("F1"));
    }

    #[test]
    fn skips_inline_images_and_comments() {
        let ops = parse(b"% w\nBI /W 1 /H 1 /BPC 8 /CS /G ID \x01w EI\n2 w S");
        let names: Vec<_> = ops.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["BI", "w", "S"]);
    }

    #[test]
    fn decodes_hash_escapes_in_names() {
        let ops = parse(b"/Pantone#20185 cs");
        assert_eq!(ops[0].name_arg(0), Some("Pantone 185"));
    }
}
