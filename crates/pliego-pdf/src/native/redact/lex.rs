//! Lector de flujos de contenido: parte el flujo en instrucciones (operandos + operador) conservando el rango de
//! bytes de cada una, para poder copiar tal cual lo que no cambia y reescribir solo lo que toca.

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Operand {
    Num(f64),
    Name(Vec<u8>),
    Str(Vec<u8>),
    Array(Vec<Operand>),
    Dict(Vec<(Vec<u8>, Operand)>),
    Bool(bool),
    Null,
}

impl Operand {
    pub(super) fn num(&self) -> Option<f64> {
        match self {
            Operand::Num(n) => Some(*n),
            _ => None,
        }
    }
    pub(super) fn name(&self) -> Option<&[u8]> {
        match self {
            Operand::Name(n) => Some(n),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Instr {
    pub(super) op: Vec<u8>,
    pub(super) operands: Vec<Operand>,
    /// Rango de bytes de la instrucción completa (desde el primer operando hasta el operador, o `EI`).
    pub(super) start: usize,
    pub(super) end: usize,
}

fn is_ws(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

fn is_delim(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

fn is_regular(b: u8) -> bool {
    !is_ws(b) && !is_delim(b)
}

struct Lexer<'a> {
    s: &'a [u8],
    pos: usize,
}

enum Tok {
    Operand(Operand),
    Keyword(Vec<u8>),
    ArrayEnd,
    DictEnd,
    /// Delimitador suelto que no significa nada (`)`, `{`…).
    Junk,
    Eof,
}

impl<'a> Lexer<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.s.len() {
            let b = self.s[self.pos];
            if is_ws(b) {
                self.pos += 1;
            } else if b == b'%' {
                while self.pos < self.s.len() && !matches!(self.s[self.pos], 10 | 13) {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn literal_string(&mut self) -> Vec<u8> {
        // `pos` está justo después del «(».
        let mut out = Vec::new();
        let mut depth = 1usize;
        while self.pos < self.s.len() {
            let b = self.s[self.pos];
            self.pos += 1;
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
                    let Some(&n) = self.s.get(self.pos) else { break };
                    self.pos += 1;
                    match n {
                        b'n' => out.push(10),
                        b'r' => out.push(13),
                        b't' => out.push(9),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'0'..=b'7' => {
                            let mut v = u32::from(n - b'0');
                            for _ in 0..2 {
                                match self.s.get(self.pos) {
                                    Some(&d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xFF) as u8);
                        }
                        13 => {
                            if self.s.get(self.pos) == Some(&10) {
                                self.pos += 1;
                            }
                        }
                        10 => {}
                        other => out.push(other),
                    }
                }
                13 => {
                    if self.s.get(self.pos) == Some(&10) {
                        self.pos += 1;
                    }
                    out.push(10);
                }
                _ => out.push(b),
            }
        }
        out
    }

    fn hex_string(&mut self) -> Vec<u8> {
        // `pos` está justo después del «<».
        let mut out = Vec::new();
        let mut hi: Option<u8> = None;
        while self.pos < self.s.len() {
            let b = self.s[self.pos];
            self.pos += 1;
            if b == b'>' {
                break;
            }
            let v = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => continue,
            };
            match hi.take() {
                Some(h) => out.push(h << 4 | v),
                None => hi = Some(v),
            }
        }
        if let Some(h) = hi {
            out.push(h << 4);
        }
        out
    }

    fn name(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        while self.pos < self.s.len() && is_regular(self.s[self.pos]) {
            let b = self.s[self.pos];
            self.pos += 1;
            if b == b'#'
                && self.pos + 1 < self.s.len()
                && let (Some(h), Some(l)) = (
                    self.s.get(self.pos).and_then(|c| (*c as char).to_digit(16)),
                    self.s.get(self.pos + 1).and_then(|c| (*c as char).to_digit(16)),
                )
            {
                out.push((h * 16 + l) as u8);
                self.pos += 2;
            } else {
                out.push(b);
            }
        }
        out
    }

    fn next(&mut self, depth: usize) -> Tok {
        self.skip_ws();
        let Some(&b) = self.s.get(self.pos) else {
            return Tok::Eof;
        };
        match b {
            b'(' => {
                self.pos += 1;
                Tok::Operand(Operand::Str(self.literal_string()))
            }
            b'<' => {
                if self.s.get(self.pos + 1) == Some(&b'<') {
                    self.pos += 2;
                    Tok::Operand(Operand::Dict(self.dict(depth + 1)))
                } else {
                    self.pos += 1;
                    Tok::Operand(Operand::Str(self.hex_string()))
                }
            }
            b'>' => {
                if self.s.get(self.pos + 1) == Some(&b'>') {
                    self.pos += 2;
                    Tok::DictEnd
                } else {
                    self.pos += 1;
                    Tok::Junk
                }
            }
            b'[' => {
                self.pos += 1;
                Tok::Operand(Operand::Array(self.array(depth + 1)))
            }
            b']' => {
                self.pos += 1;
                Tok::ArrayEnd
            }
            b'/' => {
                self.pos += 1;
                Tok::Operand(Operand::Name(self.name()))
            }
            b'{' | b'}' | b')' => {
                self.pos += 1;
                Tok::Junk
            }
            _ => {
                let start = self.pos;
                while self.pos < self.s.len() && is_regular(self.s[self.pos]) {
                    self.pos += 1;
                }
                let word = &self.s[start..self.pos];
                if let Some(n) = parse_number(word) {
                    Tok::Operand(Operand::Num(n))
                } else {
                    match word {
                        b"true" => Tok::Operand(Operand::Bool(true)),
                        b"false" => Tok::Operand(Operand::Bool(false)),
                        b"null" => Tok::Operand(Operand::Null),
                        _ => Tok::Keyword(word.to_vec()),
                    }
                }
            }
        }
    }

    fn array(&mut self, depth: usize) -> Vec<Operand> {
        let mut v = Vec::new();
        if depth > 64 {
            return v;
        }
        loop {
            match self.next(depth) {
                Tok::Operand(o) => v.push(o),
                Tok::ArrayEnd | Tok::Eof => break,
                Tok::DictEnd | Tok::Junk | Tok::Keyword(_) => {}
            }
        }
        v
    }

    fn dict(&mut self, depth: usize) -> Vec<(Vec<u8>, Operand)> {
        let mut items = Vec::new();
        if depth > 64 {
            return items;
        }
        let mut key: Option<Vec<u8>> = None;
        loop {
            match self.next(depth) {
                Tok::Operand(o) => match key.take() {
                    Some(k) => items.push((k, o)),
                    None => {
                        if let Operand::Name(n) = o {
                            key = Some(n);
                        }
                    }
                },
                Tok::DictEnd | Tok::Eof => break,
                Tok::ArrayEnd | Tok::Junk | Tok::Keyword(_) => {}
            }
        }
        items
    }
}

fn parse_number(w: &[u8]) -> Option<f64> {
    if w.is_empty() {
        return None;
    }
    let mut i = 0;
    let mut neg = false;
    while i < w.len() && (w[i] == b'+' || w[i] == b'-') {
        if w[i] == b'-' {
            neg = true;
        }
        i += 1;
    }
    let mut int = 0f64;
    let mut digits = 0;
    while i < w.len() && w[i].is_ascii_digit() {
        int = int * 10.0 + f64::from(w[i] - b'0');
        digits += 1;
        i += 1;
    }
    let mut frac = 0f64;
    if i < w.len() && w[i] == b'.' {
        i += 1;
        let mut scale = 0.1;
        while i < w.len() && w[i].is_ascii_digit() {
            frac += f64::from(w[i] - b'0') * scale;
            scale /= 10.0;
            digits += 1;
            i += 1;
        }
    }
    if i != w.len() || digits == 0 {
        return None;
    }
    let v = int + frac;
    Some(if neg { -v } else { v })
}

/// Datos de una imagen en línea: devuelve (datos, posición tras `EI`). `pos` apunta justo tras `ID`.
fn inline_data(s: &[u8], mut pos: usize, dict: &[(Vec<u8>, Operand)]) -> (Vec<u8>, usize) {
    // Un único espacio en blanco separa «ID» de los datos.
    if s.get(pos) == Some(&13) && s.get(pos + 1) == Some(&10) {
        pos += 2;
    } else if s.get(pos).is_some_and(|b| is_ws(*b)) {
        pos += 1;
    }
    let start = pos;
    let get = |long: &[u8], short: &[u8]| {
        dict.iter()
            .find(|(k, _)| k == long || k == short)
            .map(|(_, v)| v.clone())
    };
    let has_filter = get(b"Filter", b"F").is_some_and(|f| !matches!(f, Operand::Array(ref a) if a.is_empty()));
    // Sin filtro se conoce el tamaño exacto.
    if !has_filter && let (Some(Operand::Num(w)), Some(Operand::Num(h))) = (get(b"Width", b"W"), get(b"Height", b"H")) {
        let is_mask = matches!(get(b"ImageMask", b"IM"), Some(Operand::Bool(true)));
        let bpc = if is_mask {
            1.0
        } else {
            get(b"BitsPerComponent", b"BPC").and_then(|o| o.num()).unwrap_or(8.0)
        };
        let comps = if is_mask {
            Some(1.0)
        } else {
            match get(b"ColorSpace", b"CS") {
                Some(Operand::Name(n)) => match n.as_slice() {
                    b"G" | b"DeviceGray" | b"CalGray" | b"I" | b"Indexed" => Some(1.0),
                    b"RGB" | b"DeviceRGB" | b"CalRGB" => Some(3.0),
                    b"CMYK" | b"DeviceCMYK" => Some(4.0),
                    _ => None,
                },
                Some(Operand::Array(a))
                    if a.first()
                        .and_then(|f| f.name())
                        .is_some_and(|f| f == b"I" || f == b"Indexed") =>
                {
                    Some(1.0)
                }
                _ => None,
            }
        };
        if let Some(c) = comps {
            let row = ((w * c * bpc) / 8.0).ceil();
            let len = (row * h) as usize;
            if w > 0.0 && h > 0.0 && start + len <= s.len() {
                // Comprueba que sigue `EI`.
                let mut p = start + len;
                while p < s.len() && is_ws(s[p]) {
                    p += 1;
                }
                if s.get(p..p + 2) == Some(b"EI") && s.get(p + 2).is_none_or(|b| !is_regular(*b)) {
                    return (s[start..start + len].to_vec(), p + 2);
                }
            }
        }
    }
    // Con filtro: se busca «EI» rodeado de espacios y seguido de algo que parezca un operador o el final.
    let mut i = start;
    while i + 1 < s.len() {
        if s[i] == b'E'
            && s[i + 1] == b'I'
            && (i == start || is_ws(s[i - 1]))
            && s.get(i + 2).is_none_or(|b| is_ws(*b))
            && looks_like_content(&s[(i + 2).min(s.len())..])
        {
            let end = if i > start && is_ws(s[i - 1]) { i - 1 } else { i };
            return (s[start..end].to_vec(), i + 2);
        }
        i += 1;
    }
    (s[start.min(s.len())..].to_vec(), s.len())
}

/// Heurística: lo que sigue a un posible `EI` son tokens de contenido normales (no binarios).
fn looks_like_content(rest: &[u8]) -> bool {
    rest.iter()
        .take(16)
        .all(|b| *b == 9 || *b == 10 || *b == 13 || (32..127).contains(b))
}

/// Parte el flujo en instrucciones.
pub(super) fn parse(content: &[u8]) -> Vec<Instr> {
    let mut lx = Lexer { s: content, pos: 0 };
    let mut out = Vec::new();
    let mut operands: Vec<Operand> = Vec::new();
    let mut start: Option<usize> = None;
    loop {
        lx.skip_ws();
        let tok_start = lx.pos;
        match lx.next(0) {
            Tok::Eof => break,
            Tok::Operand(o) => {
                start.get_or_insert(tok_start);
                operands.push(o);
            }
            Tok::Keyword(k) => {
                let s0 = start.take().unwrap_or(tok_start);
                if k == b"BI" {
                    // Pares clave-valor hasta «ID».
                    let mut dict = Vec::new();
                    let mut key: Option<Vec<u8>> = None;
                    loop {
                        match lx.next(0) {
                            Tok::Keyword(kw) if kw == b"ID" => break,
                            Tok::Eof => break,
                            Tok::Operand(o) => match key.take() {
                                Some(k) => dict.push((k, o)),
                                None => {
                                    if let Operand::Name(n) = o {
                                        key = Some(n);
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                    let (_data, end) = inline_data(content, lx.pos, &dict);
                    lx.pos = end;
                    out.push(Instr {
                        op: k,
                        operands: Vec::new(),
                        start: s0,
                        end,
                    });
                } else {
                    out.push(Instr {
                        op: k,
                        operands: std::mem::take(&mut operands),
                        start: s0,
                        end: lx.pos,
                    });
                }
                operands.clear();
            }
            Tok::ArrayEnd | Tok::DictEnd | Tok::Junk => {}
        }
    }
    out
}

/// Escribe números sin ceros sobrantes.
pub(super) fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.5}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Cadena literal PDF con escapes.
pub(super) fn write_string(out: &mut Vec<u8>, s: &[u8]) {
    out.push(b'(');
    for &b in s {
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            10 => out.extend_from_slice(b"\\n"),
            13 => out.extend_from_slice(b"\\r"),
            _ => out.push(b),
        }
    }
    out.push(b')');
}

pub(super) fn write_name(out: &mut Vec<u8>, n: &[u8]) {
    out.push(b'/');
    for &b in n {
        if is_regular(b) && b != b'#' && (33..127).contains(&b) {
            out.push(b);
        } else {
            out.extend_from_slice(format!("#{b:02X}").as_bytes());
        }
    }
}

pub(super) fn write_operand(out: &mut Vec<u8>, o: &Operand) {
    match o {
        Operand::Num(n) => out.extend_from_slice(fmt_num(*n).as_bytes()),
        Operand::Name(n) => write_name(out, n),
        Operand::Str(s) => write_string(out, s),
        Operand::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Operand::Null => out.extend_from_slice(b"null"),
        Operand::Array(a) => {
            out.push(b'[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                write_operand(out, x);
            }
            out.push(b']');
        }
        Operand::Dict(d) => {
            out.extend_from_slice(b"<<");
            for (k, v) in d {
                write_name(out, k);
                out.push(b' ');
                write_operand(out, v);
                out.push(b' ');
            }
            out.extend_from_slice(b">>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_instructions_and_keeps_ranges() {
        let s = b"q 1 0 0 1 10 20 cm\nBT /F1 12 Tf (Hola \\(x\\)) Tj [(a) -20 (b)] TJ ET Q";
        let v = parse(s);
        let ops: Vec<_> = v.iter().map(|i| String::from_utf8_lossy(&i.op).into_owned()).collect();
        assert_eq!(ops, ["q", "cm", "BT", "Tf", "Tj", "TJ", "ET", "Q"]);
        assert_eq!(&s[v[1].start..v[1].end], b"1 0 0 1 10 20 cm");
        assert_eq!(v[4].operands, vec![Operand::Str(b"Hola (x)".to_vec())]);
        assert_eq!(
            v[5].operands,
            vec![Operand::Array(vec![
                Operand::Str(b"a".to_vec()),
                Operand::Num(-20.0),
                Operand::Str(b"b".to_vec())
            ])]
        );
    }

    #[test]
    fn hex_octal_and_numbers() {
        let v = parse(b"<48656C6C6F> Tj (\\101\\102) Tj -.5 +3. 2 re");
        assert_eq!(v[0].operands, vec![Operand::Str(b"Hello".to_vec())]);
        assert_eq!(v[1].operands, vec![Operand::Str(b"AB".to_vec())]);
        assert_eq!(
            v[2].operands,
            vec![Operand::Num(-0.5), Operand::Num(3.0), Operand::Num(2.0)]
        );
    }

    #[test]
    fn inline_image_with_binary_data() {
        let mut s = b"q BI /W 2 /H 2 /BPC 8 /CS /G ID ".to_vec();
        s.extend_from_slice(&[b'E', b'I', 0, 255]);
        s.extend_from_slice(b"\nEI Q");
        let v = parse(&s);
        assert_eq!(v.len(), 3);
        assert_eq!(v[2].op, b"Q");
    }

    #[test]
    fn inline_image_with_filter_scans_for_ei() {
        let v = parse(b"BI /W 1 /H 1 /F /AHx ID 00> EI\n0 0 m");
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].op, b"m");
    }
}
