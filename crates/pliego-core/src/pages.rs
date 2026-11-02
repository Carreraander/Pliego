use std::fmt;
use std::str::FromStr;

use crate::{Error, Result};

/// Rango de páginas tal como lo escribe una persona: `"1-3,5,8-"`, `"all"`, `"odd"`, `"even"`, `"last"`.
/// Números desde 1 en el texto; [`PageRange::resolve`] devuelve índices desde 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRange {
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    All,
    Odd,
    Even,
    /// Inclusivo, desde 1. `None` en el final significa "hasta la última".
    Span(Spec, Option<Spec>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spec {
    Num(u32),
    Last,
}

impl Spec {
    fn get(self, count: u32) -> u32 {
        match self {
            Spec::Num(n) => n,
            Spec::Last => count,
        }
    }
}

impl PageRange {
    pub fn all() -> Self {
        Self { parts: vec![Part::All] }
    }

    /// Índices desde 0, en el orden escrito, sin repetir. Error si alguna página no existe.
    pub fn resolve(&self, page_count: u32) -> Result<Vec<u32>> {
        let mut out = Vec::new();
        let push = |i: u32, out: &mut Vec<u32>| {
            if !out.contains(&i) {
                out.push(i);
            }
        };
        for part in &self.parts {
            match *part {
                Part::All => (0..page_count).for_each(|i| push(i, &mut out)),
                Part::Odd => (0..page_count).step_by(2).for_each(|i| push(i, &mut out)),
                Part::Even => (1..page_count).step_by(2).for_each(|i| push(i, &mut out)),
                Part::Span(a, b) => {
                    let a = a.get(page_count);
                    let b = b.map_or(page_count, |b| b.get(page_count));
                    if a == 0 || b == 0 || a > page_count || b > page_count {
                        return Err(Error::InvalidInput(format!(
                            "la página {} no existe (el documento tiene {page_count})",
                            a.max(b)
                        )));
                    }
                    if a <= b {
                        (a..=b).for_each(|p| push(p - 1, &mut out));
                    } else {
                        (b..=a).rev().for_each(|p| push(p - 1, &mut out));
                    }
                }
            }
        }
        Ok(out)
    }
}

fn parse_spec(s: &str) -> Result<Spec> {
    match s.trim() {
        "last" | "z" => Ok(Spec::Last),
        n => n
            .parse::<u32>()
            .map(Spec::Num)
            .map_err(|_| Error::InvalidInput(format!("número de página no válido: {n:?}"))),
    }
}

impl FromStr for PageRange {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut parts = Vec::new();
        for raw in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let part = match raw {
                "all" | "todas" => Part::All,
                "odd" | "impares" => Part::Odd,
                "even" | "pares" => Part::Even,
                _ => match raw.split_once('-') {
                    Some((a, "")) => Part::Span(parse_spec(a)?, None),
                    Some((a, b)) => Part::Span(parse_spec(a)?, Some(parse_spec(b)?)),
                    None => {
                        let n = parse_spec(raw)?;
                        Part::Span(n, Some(n))
                    }
                },
            };
            parts.push(part);
        }
        if parts.is_empty() {
            return Err(Error::InvalidInput("rango de páginas vacío".into()));
        }
        Ok(Self { parts })
    }
}

impl fmt::Display for PageRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spec = |s: Spec| match s {
            Spec::Num(n) => n.to_string(),
            Spec::Last => "last".into(),
        };
        let parts: Vec<String> = self
            .parts
            .iter()
            .map(|p| match p {
                Part::All => "all".into(),
                Part::Odd => "odd".into(),
                Part::Even => "even".into(),
                Part::Span(a, Some(b)) if a == b => spec(*a),
                Part::Span(a, Some(b)) => format!("{}-{}", spec(*a), spec(*b)),
                Part::Span(a, None) => format!("{}-", spec(*a)),
            })
            .collect();
        f.write_str(&parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str, n: u32) -> Vec<u32> {
        s.parse::<PageRange>().unwrap().resolve(n).unwrap()
    }

    #[test]
    fn basic_ranges() {
        assert_eq!(r("1-3,5", 10), vec![0, 1, 2, 4]);
        assert_eq!(r("8-", 10), vec![7, 8, 9]);
        assert_eq!(r("last", 4), vec![3]);
        assert_eq!(r("3-1", 5), vec![2, 1, 0]);
        assert_eq!(r("odd", 5), vec![0, 2, 4]);
        assert_eq!(r("even", 5), vec![1, 3]);
        assert_eq!(r("1,1,2", 3), vec![0, 1]);
    }

    #[test]
    fn errors() {
        assert!("".parse::<PageRange>().is_err());
        assert!("a-b".parse::<PageRange>().is_err());
        assert!("5".parse::<PageRange>().unwrap().resolve(3).is_err());
        assert!("0".parse::<PageRange>().unwrap().resolve(3).is_err());
    }

    #[test]
    fn display_roundtrip() {
        let p: PageRange = "1-3,5,8-,last".parse().unwrap();
        assert_eq!(p.to_string(), "1-3,5,8-,last");
    }
}
