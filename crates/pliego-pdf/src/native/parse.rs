//! Lectura y escritura de objetos PDF en sintaxis de texto (`<< /Type /Page >>`, `[0 0 612 792]`, `<4142>`…).
//!
//! lopdf no expone su analizador ni su serializador de objetos sueltos, y la capa los necesita
//! (`new_object_from_str`, `to_pdf_string`).

use lopdf::{Dictionary, Object, StringFormat};
use pliego_core::{Error, Result};

const MAX_DEPTH: usize = 64;

pub(crate) fn parse_object(src: &str) -> Result<Object> {
    let mut p = Parser {
        s: src.as_bytes(),
        i: 0,
    };
    let o = p.object(0)?;
    p.skip_ws();
    if p.i != p.s.len() {
        return Err(bad("texto sobrante tras el objeto"));
    }
    Ok(o)
}

fn bad(msg: &str) -> Error {
    Error::Pdf(format!("objeto PDF no válido: {msg}"))
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0c | 0)
}

fn is_delim(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if is_ws(b) {
                self.i += 1;
            } else if b == b'%' {
                while let Some(c) = self.peek() {
                    self.i += 1;
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Object> {
        if depth > MAX_DEPTH {
            return Err(bad("anidamiento excesivo"));
        }
        self.skip_ws();
        match self.peek().ok_or_else(|| bad("fin inesperado"))? {
            b'/' => Ok(Object::Name(self.name()?)),
            b'(' => self.literal_string(),
            b'<' => {
                if self.s.get(self.i + 1) == Some(&b'<') {
                    self.dict(depth)
                } else {
                    self.hex_string()
                }
            }
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        None => return Err(bad("matriz sin cerrar")),
                        _ => items.push(self.object(depth + 1)?),
                    }
                }
                Ok(Object::Array(items))
            }
            _ => self.keyword_or_number(),
        }
    }

    fn name(&mut self) -> Result<Vec<u8>> {
        self.i += 1; // '/'
        let mut out = Vec::new();
        while let Some(b) = self.peek() {
            if is_ws(b) || is_delim(b) {
                break;
            }
            self.i += 1;
            if b == b'#' && self.i + 1 < self.s.len() {
                let hi = self.s.get(self.i).copied().and_then(hex_val);
                let lo = self.s.get(self.i + 1).copied().and_then(hex_val);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push(h << 4 | l);
                    self.i += 2;
                    continue;
                }
            }
            out.push(b);
        }
        Ok(out)
    }

    fn literal_string(&mut self) -> Result<Object> {
        self.i += 1; // '('
        let mut out = Vec::new();
        let mut depth = 1usize;
        loop {
            let b = self.peek().ok_or_else(|| bad("cadena sin cerrar"))?;
            self.i += 1;
            match b {
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(b);
                }
                b'\\' => {
                    let c = self.peek().ok_or_else(|| bad("escape sin terminar"))?;
                    self.i += 1;
                    match c {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'0'..=b'7' => {
                            let mut v = u32::from(c - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(v as u8);
                        }
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.i += 1;
                            }
                        }
                        b'\n' => {}
                        other => out.push(other),
                    }
                }
                _ => out.push(b),
            }
        }
        Ok(Object::String(out, StringFormat::Literal))
    }

    fn hex_string(&mut self) -> Result<Object> {
        self.i += 1; // '<'
        let mut digits = Vec::new();
        loop {
            let b = self.peek().ok_or_else(|| bad("cadena hexadecimal sin cerrar"))?;
            self.i += 1;
            match b {
                b'>' => break,
                b if is_ws(b) => {}
                b => digits.push(hex_val(b).ok_or_else(|| bad("dígito hexadecimal no válido"))?),
            }
        }
        if digits.len() % 2 == 1 {
            digits.push(0);
        }
        let bytes = digits.chunks(2).map(|p| p[0] << 4 | p[1]).collect();
        Ok(Object::String(bytes, StringFormat::Hexadecimal))
    }

    fn dict(&mut self, depth: usize) -> Result<Object> {
        self.i += 2; // '<<'
        let mut d = Dictionary::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'>') if self.s.get(self.i + 1) == Some(&b'>') => {
                    self.i += 2;
                    break;
                }
                Some(b'/') => {
                    let key = self.name()?;
                    let value = self.object(depth + 1)?;
                    d.set(key, value);
                }
                _ => return Err(bad("diccionario mal formado")),
            }
        }
        Ok(Object::Dictionary(d))
    }

    fn token(&mut self) -> &[u8] {
        let start = self.i;
        while let Some(b) = self.peek() {
            if is_ws(b) || is_delim(b) {
                break;
            }
            self.i += 1;
        }
        &self.s[start..self.i]
    }

    fn keyword_or_number(&mut self) -> Result<Object> {
        let start = self.i;
        let tok = self.token().to_vec();
        if tok.is_empty() {
            return Err(bad("carácter inesperado"));
        }
        match tok.as_slice() {
            b"null" => return Ok(Object::Null),
            b"true" => return Ok(Object::Boolean(true)),
            b"false" => return Ok(Object::Boolean(false)),
            _ => {}
        }
        let text = std::str::from_utf8(&tok).map_err(|_| bad("número no válido"))?;
        if let Ok(n) = text.parse::<i64>() {
            // ¿Referencia `n g R`?
            if n >= 0 {
                let save = self.i;
                self.skip_ws();
                let gen_tok = self.token().to_vec();
                if let Ok(gen_s) = std::str::from_utf8(&gen_tok)
                    && let Ok(g) = gen_s.parse::<u16>()
                {
                    self.skip_ws();
                    let r = self.token().to_vec();
                    if r == b"R" {
                        return Ok(Object::Reference((n as u32, g)));
                    }
                }
                self.i = save;
            }
            return Ok(Object::Integer(n));
        }
        if let Ok(f) = text.parse::<f32>()
            && f.is_finite()
        {
            return Ok(Object::Real(f));
        }
        self.i = start;
        Err(bad("elemento no reconocido"))
    }
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Sintaxis PDF de un objeto, compacta como la de MuPDF (`<</A 1/B[1 2.5(x)]>>`). Los flujos se muestran como su
/// diccionario seguido de `stream`.
pub(crate) fn object_to_string(o: &Object) -> String {
    let mut s = String::new();
    write_obj(o, &mut s, 0);
    s
}

/// Añade `tok` separándolo con un espacio solo si los dos extremos serían un mismo token.
fn push_token(out: &mut String, tok: &str) {
    let regular = |c: u8| !is_ws(c) && !is_delim(c);
    if let (Some(&last), Some(&first)) = (out.as_bytes().last(), tok.as_bytes().first())
        && regular(last)
        && regular(first)
    {
        out.push(' ');
    }
    out.push_str(tok);
}

fn write_obj(o: &Object, out: &mut String, depth: usize) {
    use std::fmt::Write as _;
    if depth > MAX_DEPTH {
        push_token(out, "null");
        return;
    }
    match o {
        Object::Null => push_token(out, "null"),
        Object::Boolean(b) => push_token(out, if *b { "true" } else { "false" }),
        Object::Integer(i) => push_token(out, &i.to_string()),
        Object::Real(f) => push_token(out, &f.to_string()),
        Object::Name(n) => write_name(n, out),
        Object::String(bytes, _) => {
            if bytes.iter().all(|b| (0x20..0x7f).contains(b)) {
                let mut t = String::from("(");
                for &b in bytes {
                    if matches!(b, b'(' | b')' | b'\\') {
                        t.push('\\');
                    }
                    t.push(b as char);
                }
                t.push(')');
                push_token(out, &t);
            } else {
                let mut t = String::from("<");
                for b in bytes {
                    let _ = write!(t, "{b:02X}");
                }
                t.push('>');
                push_token(out, &t);
            }
        }
        Object::Array(a) => {
            out.push('[');
            for v in a {
                write_obj(v, out, depth + 1);
            }
            out.push(']');
        }
        Object::Dictionary(d) => write_dict(d, out, depth),
        Object::Stream(s) => {
            write_dict(&s.dict, out, depth);
            out.push_str("stream");
        }
        Object::Reference((n, g)) => push_token(out, &format!("{n} {g} R")),
    }
}

fn write_dict(d: &Dictionary, out: &mut String, depth: usize) {
    out.push_str("<<");
    for (k, v) in d {
        write_name(k, out);
        write_obj(v, out, depth + 1);
    }
    out.push_str(">>");
}

fn write_name(n: &[u8], out: &mut String) {
    use std::fmt::Write as _;
    let mut t = String::from("/");
    for &b in n {
        if is_ws(b) || is_delim(b) || b == b'#' || !(33..=126).contains(&b) {
            let _ = write!(t, "#{b:02X}");
        } else {
            t.push(b as char);
        }
    }
    out.push_str(&t);
}
