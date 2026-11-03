//! Lector XML mínimo para XFDF: elementos, atributos, texto y entidades. Sin DTD ni espacios de nombres.

use pliego_core::{Error, Result};

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    pub text: String,
}

impl Node {
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }
    pub fn child(&self, k: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == k)
    }
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(j) = rest.find(';') else { break };
        let ent = &rest[1..j];
        match ent {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = ent
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()));
                if let Some(c) = code.and_then(char::from_u32) {
                    out.push(c);
                }
            }
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

fn bad(msg: &str) -> Error {
    Error::InvalidInput(format!("XML no válido: {msg}"))
}

pub fn parse(src: &str) -> Result<Node> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Node> = vec![Node::default()];
    while i < b.len() {
        if b[i] != b'<' {
            let j = src[i..].find('<').map_or(b.len(), |k| i + k);
            let t = unescape(&src[i..j]);
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&t);
            }
            i = j;
            continue;
        }
        if src[i..].starts_with("<?") {
            i += src[i..].find("?>").ok_or_else(|| bad("declaración sin cerrar"))? + 2;
        } else if src[i..].starts_with("<!--") {
            i += src[i..].find("-->").ok_or_else(|| bad("comentario sin cerrar"))? + 3;
        } else if src[i..].starts_with("<![CDATA[") {
            let end = src[i..].find("]]>").ok_or_else(|| bad("CDATA sin cerrar"))?;
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&src[i + 9..i + end]);
            }
            i += end + 3;
        } else if src[i..].starts_with("<!") {
            i += src[i..].find('>').ok_or_else(|| bad("DOCTYPE sin cerrar"))? + 1;
        } else if src[i..].starts_with("</") {
            let end = src[i..].find('>').ok_or_else(|| bad("etiqueta sin cerrar"))?;
            let name = src[i + 2..i + end].trim();
            let node = stack.pop().ok_or_else(|| bad("etiqueta de cierre sobrante"))?;
            if node.name != name || stack.is_empty() {
                return Err(bad(&format!("se esperaba </{}>", node.name)));
            }
            stack.last_mut().unwrap().children.push(node);
            i += end + 1;
        } else {
            // Etiqueta de apertura: buscar '>' fuera de comillas.
            let mut j = i + 1;
            let mut q = None;
            while j < b.len() {
                match (b[j], q) {
                    (c @ (b'"' | b'\''), None) => q = Some(c),
                    (c, Some(qc)) if c == qc => q = None,
                    (b'>', None) => break,
                    _ => {}
                }
                j += 1;
            }
            if j >= b.len() {
                return Err(bad("etiqueta sin cerrar"));
            }
            let inner = &src[i + 1..j];
            let (inner, selfclose) = match inner.strip_suffix('/') {
                Some(x) => (x, true),
                None => (inner, false),
            };
            let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
            let mut node = Node {
                name: inner[..name_end].to_string(),
                ..Default::default()
            };
            let mut rest = inner[name_end..].trim_start();
            while !rest.is_empty() {
                let eq = rest.find('=').ok_or_else(|| bad("atributo sin valor"))?;
                let key = rest[..eq].trim().to_string();
                let after = rest[eq + 1..].trim_start();
                let qc = after
                    .chars()
                    .next()
                    .filter(|c| *c == '"' || *c == '\'')
                    .ok_or_else(|| bad("atributo sin comillas"))?;
                let close = after[1..].find(qc).ok_or_else(|| bad("comillas sin cerrar"))?;
                node.attrs.push((key, unescape(&after[1..1 + close])));
                rest = after[close + 2..].trim_start();
            }
            if selfclose {
                stack.last_mut().unwrap().children.push(node);
            } else {
                stack.push(node);
            }
            i = j + 1;
        }
    }
    if stack.len() != 1 {
        return Err(bad("etiquetas sin cerrar"));
    }
    stack
        .pop()
        .unwrap()
        .children
        .into_iter()
        .next()
        .ok_or_else(|| bad("documento vacío"))
}
