//! Lectura mínima de flujos de contenido: operadores y colocación de imágenes.
//!
//! No interpreta todo el lenguaje: solo lo necesario para seguir la matriz de transformación (`q`, `Q`, `cm`)
//! y localizar los operadores `Do`. Las imágenes dentro de XObjects de formulario no se tienen en cuenta.

use mupdf::Matrix;

#[derive(Debug, Clone)]
pub enum Operand {
    Num(f32),
    Name(String),
    Other,
}

#[derive(Debug, Clone)]
pub struct Op {
    pub operator: String,
    pub operands: Vec<Operand>,
    /// Inicio del primer operando (o del operador si no tiene).
    pub start: usize,
    pub end: usize,
}

fn is_ws(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

fn is_delim(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

fn skip_string(s: &[u8], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < s.len() {
        match s[i] {
            b'\\' => i += 1,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    s.len()
}

/// Salta una imagen en línea (`BI ... ID datos EI`) y devuelve el final.
fn skip_inline_image(s: &[u8], from: usize) -> usize {
    let mut i = from;
    while i + 1 < s.len() {
        if is_ws(s[i]) && s[i + 1] == b'I' && s.get(i + 2) == Some(&b'D') && s.get(i + 3).is_none_or(|b| is_ws(*b)) {
            i += 4;
            break;
        }
        i += 1;
    }
    while i + 2 < s.len() {
        if is_ws(s[i]) && s[i + 1] == b'E' && s[i + 2] == b'I' && s.get(i + 3).is_none_or(|b| is_ws(*b)) {
            return i + 3;
        }
        i += 1;
    }
    s.len()
}

pub fn parse(s: &[u8]) -> Vec<Op> {
    let mut ops = Vec::new();
    let mut operands: Vec<Operand> = Vec::new();
    let mut first: Option<usize> = None;
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        if is_ws(b) {
            i += 1;
            continue;
        }
        if b == b'%' {
            while i < s.len() && s[i] != b'\n' && s[i] != b'\r' {
                i += 1;
            }
            continue;
        }
        let start = i;
        let mut push = |op: Operand, first: &mut Option<usize>| {
            first.get_or_insert(start);
            operands.push(op);
        };
        match b {
            b'(' => {
                i = skip_string(s, i);
                push(Operand::Other, &mut first);
            }
            b'<' if s.get(i + 1) == Some(&b'<') => {
                i += 2;
                push(Operand::Other, &mut first);
            }
            b'<' => {
                while i < s.len() && s[i] != b'>' {
                    i += 1;
                }
                i += 1;
                push(Operand::Other, &mut first);
            }
            b'>' | b'[' | b']' | b'{' | b'}' | b')' => {
                i += if b == b'>' && s.get(i + 1) == Some(&b'>') { 2 } else { 1 };
                push(Operand::Other, &mut first);
            }
            b'/' => {
                i += 1;
                while i < s.len() && !is_ws(s[i]) && !is_delim(s[i]) {
                    i += 1;
                }
                let name = String::from_utf8_lossy(&s[start + 1..i]).into_owned();
                push(Operand::Name(name), &mut first);
            }
            _ => {
                while i < s.len() && !is_ws(s[i]) && !is_delim(s[i]) {
                    i += 1;
                }
                let word = std::str::from_utf8(&s[start..i]).unwrap_or("");
                let numeric = word
                    .bytes()
                    .all(|c| c.is_ascii_digit() || matches!(c, b'+' | b'-' | b'.'));
                if numeric && !word.is_empty() {
                    push(Operand::Num(word.parse().unwrap_or(0.0)), &mut first);
                    continue;
                }
                let operator = word.to_owned();
                if operator == "BI" {
                    i = skip_inline_image(s, i);
                }
                ops.push(Op {
                    operator,
                    operands: std::mem::take(&mut operands),
                    start: first.take().unwrap_or(start),
                    end: i,
                });
            }
        }
    }
    ops
}

/// Una imagen dibujada con `Do` y la matriz vigente en ese momento (espacio de usuario por defecto).
#[derive(Debug, Clone)]
pub struct Placement {
    pub name: String,
    pub ctm: Matrix,
    /// Rango del `/Nombre Do` dentro del flujo.
    pub range: (usize, usize),
}

/// Todos los `Do` del flujo, en orden. `is_image` decide si un nombre de XObject es una imagen.
pub fn placements(ops: &[Op], is_image: impl Fn(&str) -> bool) -> Vec<Placement> {
    let mut stack: Vec<Matrix> = Vec::new();
    let mut ctm = Matrix::IDENTITY;
    let mut out = Vec::new();
    for op in ops {
        match op.operator.as_str() {
            "q" => stack.push(ctm.clone()),
            "Q" => {
                if let Some(m) = stack.pop() {
                    ctm = m;
                }
            }
            "cm" => {
                let n: Vec<f32> = op
                    .operands
                    .iter()
                    .filter_map(|o| if let Operand::Num(v) = o { Some(*v) } else { None })
                    .collect();
                if n.len() == 6 {
                    let mut m = Matrix::new(n[0], n[1], n[2], n[3], n[4], n[5]);
                    m.concat(ctm.clone());
                    ctm = m;
                }
            }
            "Do" => {
                if let Some(Operand::Name(name)) = op.operands.last()
                    && is_image(name)
                {
                    out.push(Placement {
                        name: name.clone(),
                        ctm: ctm.clone(),
                        range: (op.start, op.end),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// Un trazado vectorial pintado (relleno o trazo), con la matriz vigente y sus puntos en espacio local.
#[derive(Debug, Clone)]
pub struct PathObject {
    pub ctm: Matrix,
    /// Desde el primer operador de construcción hasta el de pintado, ambos incluidos.
    pub range: (usize, usize),
    pub points: Vec<(f32, f32)>,
}

struct Building {
    start: usize,
    points: Vec<(f32, f32)>,
    clip: bool,
}

fn nums(op: &Op) -> Vec<f32> {
    op.operands
        .iter()
        .filter_map(|o| if let Operand::Num(v) = o { Some(*v) } else { None })
        .collect()
}

/// Trazados pintados del flujo, en orden. Se omiten los que solo recortan (`W n`) o no pintan (`n`).
pub fn paths(ops: &[Op]) -> Vec<PathObject> {
    let mut stack: Vec<Matrix> = Vec::new();
    let mut ctm = Matrix::IDENTITY;
    let mut out = Vec::new();
    let mut cur: Option<Building> = None;
    for op in ops {
        let o = op.operator.as_str();
        match o {
            "q" => stack.push(ctm.clone()),
            "Q" => {
                if let Some(m) = stack.pop() {
                    ctm = m;
                }
            }
            "cm" => {
                let n = nums(op);
                if n.len() == 6 {
                    let mut m = Matrix::new(n[0], n[1], n[2], n[3], n[4], n[5]);
                    m.concat(ctm.clone());
                    ctm = m;
                }
            }
            "m" | "l" | "c" | "v" | "y" | "re" | "h" => {
                let entry = cur.get_or_insert_with(|| Building {
                    start: op.start,
                    points: Vec::new(),
                    clip: false,
                });
                let n = nums(op);
                if o == "re" && n.len() == 4 {
                    let (x, y, w, h) = (n[0], n[1], n[2], n[3]);
                    entry.points.extend([(x, y), (x + w, y), (x + w, y + h), (x, y + h)]);
                } else {
                    entry.points.extend(n.chunks_exact(2).map(|c| (c[0], c[1])));
                }
            }
            "W" | "W*" => {
                if let Some(e) = cur.as_mut() {
                    e.clip = true;
                }
            }
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                if let Some(Building { start, points, clip }) = cur.take()
                    && o != "n"
                    && !clip
                    && !points.is_empty()
                {
                    out.push(PathObject {
                        ctm: ctm.clone(),
                        range: (start, op.end),
                        points,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigue_la_matriz_y_localiza_do() {
        let s = b"q 2 0 0 2 10 20 cm (a Do) Tj q 1 0 0 1 5 5 cm /Im1 Do Q Q /Im2 Do % /Im3 Do\n";
        let ops = parse(s);
        let p = placements(&ops, |_| true);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].name, "Im1");
        assert_eq!((p[0].ctm.e, p[0].ctm.f), (20.0, 30.0));
        assert_eq!(p[1].ctm, Matrix::IDENTITY);
        assert_eq!(&s[p[0].range.0..p[0].range.1], b"/Im1 Do");
    }

    #[test]
    fn trazados_pintados_sin_recortes() {
        let s = b"0 0 100 100 re W n 1 0 0 rg q 2 0 0 2 0 0 cm 10 10 20 20 re f Q 0 0 m 5 5 l S 1 1 m n";
        let p = paths(&parse(s));
        assert_eq!(p.len(), 2);
        assert_eq!(&s[p[0].range.0..p[0].range.1], b"10 10 20 20 re f");
        assert_eq!(p[0].points.len(), 4);
        assert_eq!(&s[p[1].range.0..p[1].range.1], b"0 0 m 5 5 l S");
    }

    #[test]
    fn imagen_en_linea_no_confunde() {
        let s = b"BI /W 1 /H 1 ID \x00 Do EI /Im1 Do";
        let p = placements(&parse(s), |_| true);
        assert_eq!(p.len(), 1);
    }
}
