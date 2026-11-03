//! FDF y XFDF: intercambio de valores de campos con otros programas.
//!
//! Ambos formatos se reducen a pares `(nombre completo, valor)`. Los campos jerárquicos se aplanan con
//! puntos (`padre.hijo`), como los nombra AcroForm.

use std::fmt::Write;

use pliego_core::{Error, Result};

use crate::xml::{self, Node, escape};

pub type Pairs = Vec<(String, String)>;

// ---------------------------------------------------------------- XFDF

fn xfdf_fields(n: &Node, prefix: &str, out: &mut Pairs) {
    for f in n.children.iter().filter(|c| c.name == "field") {
        let Some(name) = f.attr("name") else { continue };
        let full = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}.{name}")
        };
        if let Some(v) = f.child("value") {
            out.push((full.clone(), v.text.clone()));
        }
        xfdf_fields(f, &full, out);
    }
}

pub fn parse_xfdf(src: &str) -> Result<Pairs> {
    let root = xml::parse(src)?;
    if root.name != "xfdf" {
        return Err(Error::InvalidInput("no es un archivo XFDF".into()));
    }
    let mut out = Vec::new();
    if let Some(fields) = root.child("fields") {
        xfdf_fields(fields, "", &mut out);
    }
    Ok(out)
}

pub fn write_xfdf(pairs: &Pairs) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n<fields>\n",
    );
    for (name, value) in pairs {
        // Anidar por los puntos del nombre.
        let parts: Vec<&str> = name.split('.').collect();
        for p in &parts {
            let _ = write!(s, "<field name=\"{}\">", escape(p));
        }
        let _ = write!(s, "<value>{}</value>", escape(value));
        for _ in &parts {
            s.push_str("</field>");
        }
        s.push('\n');
    }
    s.push_str("</fields>\n</xfdf>\n");
    s
}

// ----------------------------------------------------------------- FDF

#[derive(Debug, Clone)]
enum Obj {
    Dict(Vec<(String, Obj)>),
    Arr(Vec<Obj>),
    Str(Vec<u8>),
    Name(String),
    Other,
}

struct Lexer<'a> {
    b: &'a [u8],
    i: usize,
}

fn bad(m: &str) -> Error {
    Error::InvalidInput(format!("FDF no válido: {m}"))
}

impl Lexer<'_> {
    fn skip_ws(&mut self) {
        while self.i < self.b.len() {
            match self.b[self.i] {
                b' ' | b'\t' | b'\r' | b'\n' | 0 | 12 => self.i += 1,
                b'%' => {
                    while self.i < self.b.len() && !matches!(self.b[self.i], b'\r' | b'\n') {
                        self.i += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn string(&mut self) -> Result<Vec<u8>> {
        // Tras el '(' inicial.
        let mut depth = 1;
        let mut out = Vec::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'\\' => {
                    let Some(&e) = self.b.get(self.i) else { break };
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.b.get(self.i) {
                                    Some(&d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xff) as u8);
                        }
                        b'\r' => {
                            if self.b.get(self.i) == Some(&b'\n') {
                                self.i += 1;
                            }
                        }
                        b'\n' => {}
                        other => out.push(other),
                    }
                }
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        Err(bad("cadena sin cerrar"))
    }

    fn hex(&mut self) -> Result<Vec<u8>> {
        let mut digits = Vec::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            if c == b'>' {
                if digits.len() % 2 == 1 {
                    digits.push(0);
                }
                return Ok(digits.chunks(2).map(|p| p[0] * 16 + p[1]).collect());
            }
            if let Some(d) = (c as char).to_digit(16) {
                digits.push(d as u8);
            }
        }
        Err(bad("cadena hexadecimal sin cerrar"))
    }

    fn token(&mut self) -> String {
        let s = self.i;
        while self.i < self.b.len()
            && !matches!(
                self.b[self.i],
                b' ' | b'\t' | b'\r' | b'\n' | 0 | 12 | b'/' | b'<' | b'>' | b'[' | b']' | b'(' | b')' | b'%'
            )
        {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.b[s..self.i]).into_owned()
    }

    fn obj(&mut self, depth: u32) -> Result<Option<Obj>> {
        if depth > 64 {
            return Err(bad("anidamiento excesivo"));
        }
        self.skip_ws();
        let Some(&c) = self.b.get(self.i) else { return Ok(None) };
        match c {
            b'<' if self.b.get(self.i + 1) == Some(&b'<') => {
                self.i += 2;
                let mut d = Vec::new();
                loop {
                    self.skip_ws();
                    if self.b[self.i..].starts_with(b">>") {
                        self.i += 2;
                        return Ok(Some(Obj::Dict(d)));
                    }
                    // Restos de referencias indirectas (`1 0 R`) tras un valor: se descartan.
                    if self
                        .b
                        .get(self.i)
                        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'-' || *c == b'+')
                    {
                        self.token();
                        continue;
                    }
                    if self.b.get(self.i) != Some(&b'/') {
                        return Err(bad("se esperaba un nombre de clave"));
                    }
                    self.i += 1;
                    let k = self.token();
                    let v = self.obj(depth + 1)?.ok_or_else(|| bad("diccionario sin cerrar"))?;
                    d.push((k, v));
                }
            }
            b'<' => {
                self.i += 1;
                Ok(Some(Obj::Str(self.hex()?)))
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                loop {
                    self.skip_ws();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Some(Obj::Arr(a)));
                    }
                    a.push(self.obj(depth + 1)?.ok_or_else(|| bad("arreglo sin cerrar"))?);
                }
            }
            b'(' => {
                self.i += 1;
                Ok(Some(Obj::Str(self.string()?)))
            }
            b'/' => {
                self.i += 1;
                Ok(Some(Obj::Name(self.token())))
            }
            b'>' | b']' | b')' => Err(bad("delimitador inesperado")),
            _ => {
                let t = self.token();
                if t.is_empty() {
                    self.i += 1;
                }
                Ok(Some(Obj::Other))
            }
        }
    }
}

/// Decodifica una cadena PDF: UTF-16BE con BOM, UTF-8 con BOM o PDFDocEncoding (aprox. Latin-1).
fn decode(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest
            .chunks(2)
            .filter(|p| p.len() == 2)
            .map(|p| u16::from_be_bytes([p[0], p[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(rest).into_owned()
    } else {
        b.iter().map(|&c| c as char).collect()
    }
}

fn get<'a>(d: &'a [(String, Obj)], k: &str) -> Option<&'a Obj> {
    d.iter().find(|(n, _)| n == k).map(|(_, v)| v)
}

fn fdf_fields(arr: &[Obj], prefix: &str, out: &mut Pairs) {
    for f in arr {
        let Obj::Dict(d) = f else { continue };
        let Some(Obj::Str(t)) = get(d, "T") else { continue };
        let t = decode(t);
        let full = if prefix.is_empty() { t } else { format!("{prefix}.{t}") };
        match get(d, "V") {
            Some(Obj::Str(v)) => out.push((full.clone(), decode(v))),
            Some(Obj::Name(v)) => out.push((full.clone(), v.clone())),
            _ => {}
        }
        if let Some(Obj::Arr(kids)) = get(d, "Kids") {
            fdf_fields(kids, &full, out);
        }
    }
}

/// Lee un FDF. `bytes` son los bytes del archivo (cada carácter es un byte, vista Latin-1).
pub fn parse_fdf(bytes: &[u8]) -> Result<Pairs> {
    if !bytes.starts_with(b"%FDF") {
        return Err(Error::InvalidInput("no es un archivo FDF".into()));
    }
    let mut lx = Lexer { b: bytes, i: 0 };
    let mut out = Vec::new();
    let mut found = false;
    // Recorre los objetos de nivel superior buscando el diccionario con /FDF.
    while let Some(o) = lx.obj(0)? {
        if let Obj::Dict(d) = &o
            && let Some(Obj::Dict(fdf)) = get(d, "FDF")
        {
            found = true;
            if let Some(Obj::Arr(fields)) = get(fdf, "Fields") {
                fdf_fields(fields, "", &mut out);
            }
        }
    }
    if !found {
        return Err(Error::InvalidInput("el FDF no contiene el diccionario /FDF".into()));
    }
    Ok(out)
}

fn pdf_string(s: &str) -> String {
    if s.chars().all(|c| (' '..='~').contains(&c)) {
        let mut o = String::from("(");
        for c in s.chars() {
            if matches!(c, '(' | ')' | '\\') {
                o.push('\\');
            }
            o.push(c);
        }
        o.push(')');
        o
    } else {
        // UTF-16BE con BOM en hexadecimal: válido para cualquier carácter.
        let mut o = String::from("<FEFF");
        for u in s.encode_utf16() {
            let _ = write!(o, "{u:04X}");
        }
        o.push('>');
        o
    }
}

pub fn write_fdf(pairs: &Pairs) -> String {
    let mut s = String::from("%FDF-1.2\n1 0 obj\n<< /FDF << /Fields [\n");
    for (name, value) in pairs {
        let parts: Vec<&str> = name.split('.').collect();
        for (i, p) in parts.iter().enumerate() {
            let _ = write!(s, "<< /T {} ", pdf_string(p));
            if i + 1 < parts.len() {
                s.push_str("/Kids [ ");
            }
        }
        let _ = write!(s, "/V {} >>", pdf_string(value));
        for _ in 1..parts.len() {
            s.push_str(" ] >>");
        }
        s.push('\n');
    }
    s.push_str("] >> >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs() -> Pairs {
        vec![
            ("nombre".into(), "Ñandú (el) \\ <b> & \"x\"".into()),
            ("datos.dni".into(), "12345678Z".into()),
            ("datos.emoji".into(), "ok 😀".into()),
            ("vacío".into(), String::new()),
        ]
    }

    #[test]
    fn xfdf_roundtrip_with_hierarchy() {
        let x = write_xfdf(&pairs());
        assert!(x.contains("<field name=\"datos\"><field name=\"dni\">"), "{x}");
        assert_eq!(parse_xfdf(&x).unwrap(), pairs());
    }

    #[test]
    fn fdf_roundtrip_with_hierarchy() {
        let f = write_fdf(&pairs());
        assert!(f.is_ascii(), "{f}");
        assert_eq!(parse_fdf(f.as_bytes()).unwrap(), pairs());
    }

    #[test]
    fn fdf_reads_foreign_syntax() {
        let src = b"%FDF-1.2\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<</FDF<</F(a.pdf)/Fields[<</T(grupo)/Kids[<</T(a)/V(Hola\\040mundo\\101)>><</T<466F6F>/V/Yes>>]>>]/ID[<AA><BB>]>>>>\nendobj\ntrailer\n<</Root 1 0 R>>\n%%EOF";
        let p = parse_fdf(src).unwrap();
        assert_eq!(
            p,
            vec![
                ("grupo.a".to_string(), "Hola mundoA".to_string()),
                ("grupo.Foo".into(), "Yes".into())
            ]
        );
    }

    #[test]
    fn fdf_decodes_latin1_and_utf16() {
        let src = b"%FDF-1.2\n1 0 obj\n<< /FDF << /Fields [ << /T (a) /V (\\361) >> << /T (b) /V <FEFF00F1263A> >> ] >> >>\nendobj\n";
        let p = parse_fdf(src).unwrap();
        assert_eq!(p, vec![("a".to_string(), "ñ".to_string()), ("b".into(), "ñ☺".into())]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_fdf(b"hola").is_err());
        assert!(parse_fdf(b"%FDF-1.2\n1 0 obj\n<< /FDF << /Fields [ << /T (a ").is_err());
        assert!(parse_fdf(b"%FDF-1.2\n1 0 obj\n<< /Otro 1 >>\n").is_err());
        assert!(parse_xfdf("<html/>").is_err());
        assert!(parse_xfdf("<xfdf><fields>").is_err());
        // Anidamiento excesivo no desborda la pila.
        let deep = format!("%FDF-1.2\n{}", "[".repeat(10_000));
        assert!(parse_fdf(deep.as_bytes()).is_err());
    }
}
