//! Lector mínimo de flujos de contenido de página: reconoce objetos de texto, imágenes y trazados
//! y dónde empiezan y terminan en los bytes, para poder envolverlos en contenido marcado.

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tok {
    Num(f32),
    Name(String),
    Str(Vec<u8>),
    /// Cualquier otro operando o signo (`[`, `]`, `<<`, `>>`, booleanos...).
    Other,
    Op(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub tok: Tok,
    pub start: usize,
    pub end: usize,
}

fn is_ws(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

fn is_delim(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

fn hex_val(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

/// Divide el flujo en símbolos. Las imágenes en línea (`BI ... ID ... EI`) se devuelven como un único
/// operador `INLINE_IMAGE`.
pub(crate) fn tokenize(data: &[u8]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = data.len();
    while i < n {
        let b = data[i];
        if is_ws(b) {
            i += 1;
            continue;
        }
        let start = i;
        match b {
            b'%' => {
                while i < n && data[i] != b'\n' && data[i] != b'\r' {
                    i += 1;
                }
            }
            b'(' => {
                let mut depth = 1;
                let mut s = Vec::new();
                i += 1;
                while i < n && depth > 0 {
                    match data[i] {
                        b'\\' if i + 1 < n => {
                            i += 1;
                            match data[i] {
                                b'n' => s.push(b'\n'),
                                b'r' => s.push(b'\r'),
                                b't' => s.push(b'\t'),
                                b'b' => s.push(8),
                                b'f' => s.push(12),
                                c @ b'0'..=b'7' => {
                                    let mut v = (c - b'0') as u32;
                                    for _ in 0..2 {
                                        if i + 1 < n && (b'0'..=b'7').contains(&data[i + 1]) {
                                            i += 1;
                                            v = v * 8 + (data[i] - b'0') as u32;
                                        }
                                    }
                                    s.push(v as u8);
                                }
                                b'\n' | b'\r' => {}
                                c => s.push(c),
                            }
                        }
                        b'(' => {
                            depth += 1;
                            s.push(b'(');
                        }
                        b')' => {
                            depth -= 1;
                            if depth > 0 {
                                s.push(b')');
                            }
                        }
                        c => s.push(c),
                    }
                    i += 1;
                }
                out.push(Token {
                    tok: Tok::Str(s),
                    start,
                    end: i,
                });
            }
            b'<' if i + 1 < n && data[i + 1] == b'<' => {
                i += 2;
                out.push(Token {
                    tok: Tok::Other,
                    start,
                    end: i,
                });
            }
            b'>' if i + 1 < n && data[i + 1] == b'>' => {
                i += 2;
                out.push(Token {
                    tok: Tok::Other,
                    start,
                    end: i,
                });
            }
            b'<' => {
                i += 1;
                let mut s = Vec::new();
                let mut hi: Option<u8> = None;
                while i < n && data[i] != b'>' {
                    if let Some(v) = hex_val(data[i]) {
                        match hi.take() {
                            Some(h) => s.push(h * 16 + v),
                            None => hi = Some(v),
                        }
                    }
                    i += 1;
                }
                if let Some(h) = hi {
                    s.push(h * 16);
                }
                i = (i + 1).min(n);
                out.push(Token {
                    tok: Tok::Str(s),
                    start,
                    end: i,
                });
            }
            b'[' | b']' | b'{' | b'}' | b')' | b'>' => {
                i += 1;
                out.push(Token {
                    tok: Tok::Other,
                    start,
                    end: i,
                });
            }
            b'/' => {
                i += 1;
                while i < n && !is_ws(data[i]) && !is_delim(data[i]) {
                    i += 1;
                }
                let name = String::from_utf8_lossy(&data[start + 1..i]).into_owned();
                out.push(Token {
                    tok: Tok::Name(name),
                    start,
                    end: i,
                });
            }
            _ => {
                while i < n && !is_ws(data[i]) && !is_delim(data[i]) {
                    i += 1;
                }
                let word = std::str::from_utf8(&data[start..i]).unwrap_or("");
                if let Ok(v) = word.parse::<f32>() {
                    out.push(Token {
                        tok: Tok::Num(v),
                        start,
                        end: i,
                    });
                } else if word == "BI" {
                    // Imagen en línea: hasta «EI» rodeado de espacio.
                    let mut j = i;
                    let mut end = n;
                    while j + 1 < n {
                        if data[j] == b'E'
                            && data[j + 1] == b'I'
                            && (j == 0 || is_ws(data[j - 1]))
                            && (j + 2 >= n || is_ws(data[j + 2]))
                        {
                            end = j + 2;
                            break;
                        }
                        j += 1;
                    }
                    i = end;
                    out.push(Token {
                        tok: Tok::Op("INLINE_IMAGE".into()),
                        start,
                        end,
                    });
                } else if word == "true" || word == "false" || word == "null" {
                    out.push(Token {
                        tok: Tok::Other,
                        start,
                        end: i,
                    });
                } else {
                    out.push(Token {
                        tok: Tok::Op(word.to_string()),
                        start,
                        end: i,
                    });
                }
            }
        }
    }
    out
}

/// Clase de un trozo del flujo de contenido que se puede envolver.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SegKind {
    Text(TextInfo),
    /// `Do` de un XObject (el nombre se resuelve en los recursos).
    Xobject(String),
    InlineImage,
    /// Trazado pintado (relleno o trazo).
    Path,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct TextInfo {
    /// Tamaño efectivo de la fuente (Tf × matriz de texto × CTM).
    pub size: f32,
    pub font: Option<String>,
    /// Primera cadena mostrada.
    pub sample: Vec<u8>,
    /// Número de bytes de texto mostrados.
    pub len: usize,
    /// Posición vertical aproximada del texto (en el espacio de texto).
    pub y: Option<f32>,
    pub x: Option<f32>,
}

#[derive(Debug, Clone)]
pub(crate) struct Segment {
    pub start: usize,
    pub end: usize,
    pub kind: SegKind,
}

fn scale_of(m: &[f32; 6]) -> f32 {
    (m[0] * m[3] - m[1] * m[2]).abs().sqrt()
}

fn mul(a: &[f32; 6], b: &[f32; 6]) -> [f32; 6] {
    // a × b con la convención de PDF (a se aplica primero)
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Segmenta el flujo en objetos de texto, imágenes y trazados.
pub(crate) fn segment(data: &[u8], toks: &[Token]) -> Vec<Segment> {
    let mut segs = Vec::new();
    let mut ctm = IDENTITY;
    let mut stack: Vec<[f32; 6]> = Vec::new();
    let mut font: Option<(String, f32)> = None;
    let mut operands: Vec<usize> = Vec::new(); // índices de tokens de operandos pendientes
    let mut path_start: Option<usize> = None;
    let mut i = 0;
    while i < toks.len() {
        let Tok::Op(op) = &toks[i].tok else {
            operands.push(i);
            i += 1;
            continue;
        };
        let nums: Vec<f32> = operands
            .iter()
            .filter_map(|&k| if let Tok::Num(v) = toks[k].tok { Some(v) } else { None })
            .collect();
        let first_operand_start = operands.first().map(|&k| toks[k].start).unwrap_or(toks[i].start);
        match op.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap_or(IDENTITY),
            "cm" if nums.len() == 6 => ctm = mul(&[nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]], &ctm),
            "Tf" => {
                if let (Some(&n), Some(&s)) = (
                    operands.iter().find(|&&k| matches!(toks[k].tok, Tok::Name(_))),
                    operands.iter().rev().find(|&&k| matches!(toks[k].tok, Tok::Num(_))),
                ) && let (Tok::Name(name), Tok::Num(size)) = (&toks[n].tok, &toks[s].tok)
                {
                    font = Some((name.clone(), *size));
                }
            }
            "m" | "re" | "l" | "c" | "v" | "y" | "h" => {
                path_start.get_or_insert(first_operand_start);
            }
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                if let Some(s) = path_start.take() {
                    segs.push(Segment {
                        start: s,
                        end: toks[i].end,
                        kind: SegKind::Path,
                    });
                }
            }
            "n" => path_start = None,
            "Do" => {
                if let Some(&k) = operands.last()
                    && let Tok::Name(name) = &toks[k].tok
                {
                    segs.push(Segment {
                        start: toks[k].start,
                        end: toks[i].end,
                        kind: SegKind::Xobject(name.clone()),
                    });
                }
            }
            "INLINE_IMAGE" => segs.push(Segment {
                start: toks[i].start,
                end: toks[i].end,
                kind: SegKind::InlineImage,
            }),
            "BT" => {
                // Recorre hasta ET.
                let start = toks[i].start;
                let mut tm = IDENTITY;
                let mut info = TextInfo::default();
                let mut j = i + 1;
                let mut ops: Vec<usize> = Vec::new();
                let mut end = toks[i].end;
                let mut shown = false;
                while j < toks.len() {
                    match &toks[j].tok {
                        Tok::Op(o) => {
                            let nums: Vec<f32> = ops
                                .iter()
                                .filter_map(|&k| if let Tok::Num(v) = toks[k].tok { Some(v) } else { None })
                                .collect();
                            match o.as_str() {
                                "Tf" => {
                                    if let (Some(&n), Some(&s)) = (
                                        ops.iter().find(|&&k| matches!(toks[k].tok, Tok::Name(_))),
                                        ops.iter().rev().find(|&&k| matches!(toks[k].tok, Tok::Num(_))),
                                    ) && let (Tok::Name(name), Tok::Num(size)) = (&toks[n].tok, &toks[s].tok)
                                    {
                                        font = Some((name.clone(), *size));
                                    }
                                }
                                "Tm" if nums.len() == 6 => {
                                    tm = [nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]];
                                    if info.y.is_none() {
                                        info.x = Some(tm[4]);
                                        info.y = Some(tm[5]);
                                    }
                                }
                                "Td" | "TD" if nums.len() == 2 => {
                                    if info.y.is_none() {
                                        info.x = Some(tm[4] + nums[0]);
                                        info.y = Some(tm[5] + nums[1]);
                                    }
                                    tm[4] += nums[0];
                                    tm[5] += nums[1];
                                }
                                "Tj" | "TJ" | "'" | "\"" => {
                                    shown = true;
                                    for &k in &ops {
                                        if let Tok::Str(s) = &toks[k].tok {
                                            if info.sample.is_empty() {
                                                info.sample = s.clone();
                                            }
                                            info.len += s.len();
                                        }
                                    }
                                    if info.font.is_none()
                                        && let Some((name, size)) = &font
                                    {
                                        info.font = Some(name.clone());
                                        info.size = size.abs() * scale_of(&tm) * scale_of(&ctm);
                                    }
                                }
                                "ET" => {
                                    end = toks[j].end;
                                    break;
                                }
                                _ => {}
                            }
                            ops.clear();
                        }
                        _ => ops.push(j),
                    }
                    j += 1;
                }
                if shown {
                    segs.push(Segment {
                        start,
                        end,
                        kind: SegKind::Text(info),
                    });
                }
                i = j + 1;
                operands.clear();
                continue;
            }
            _ => {}
        }
        let _ = data;
        operands.clear();
        i += 1;
    }
    segs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_and_strings() {
        let t = tokenize(b"BT /F1 12 Tf (a\\(b\\051) Tj <48656C6C6F> Tj [(x) -20 (y)] TJ ET % comentario\nq");
        let ops: Vec<_> = t
            .iter()
            .filter_map(|t| {
                if let Tok::Op(o) = &t.tok {
                    Some(o.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(ops, ["BT", "Tf", "Tj", "Tj", "TJ", "ET", "q"]);
        let strs: Vec<_> = t
            .iter()
            .filter_map(|t| {
                if let Tok::Str(s) = &t.tok {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(strs[0], b"a(b)");
        assert_eq!(strs[1], b"Hello");
    }

    #[test]
    fn segments_text_images_and_paths() {
        let data = b"q 100 0 0 100 10 10 cm /Im1 Do Q 0 0 50 50 re f BT /F1 24 Tf 72 700 Td (Titulo) Tj ET BT /F1 10 Tf 72 680 Td (cuerpo) Tj ET 0 0 m 5 5 l n";
        let toks = tokenize(data);
        let segs = segment(data, &toks);
        assert_eq!(segs.len(), 4, "{segs:?}");
        assert!(matches!(&segs[0].kind, SegKind::Xobject(n) if n == "Im1"));
        assert_eq!(segs[1].kind, SegKind::Path);
        let SegKind::Text(a) = &segs[2].kind else { panic!() };
        assert_eq!((a.size, a.y, a.sample.clone()), (24.0, Some(700.0), b"Titulo".to_vec()));
        let SegKind::Text(b) = &segs[3].kind else { panic!() };
        assert_eq!(b.size, 10.0);
        assert_eq!(
            &data[segs[2].start..segs[2].end],
            b"BT /F1 24 Tf 72 700 Td (Titulo) Tj ET"
        );
    }

    #[test]
    fn inline_images_are_one_segment() {
        let data = b"q BI /W 1 /H 1 /BPC 8 /CS /G ID \x00 EI Q";
        let segs = segment(data, &tokenize(data));
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].kind, SegKind::InlineImage);
    }
}
