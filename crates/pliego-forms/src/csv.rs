//! CSV mínimo (RFC 4180): comillas dobles, comas, saltos de línea dentro de comillas y BOM inicial.

pub fn parse(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if cell.is_empty() => quoted = true,
            (',', false) => row.push(std::mem::take(&mut cell)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            (c, _) => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows.retain(|r| !(r.len() == 1 && r[0].is_empty()));
    rows
}

pub fn write(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for r in rows {
        let cells: Vec<String> = r
            .iter()
            .map(|c| {
                if c.contains([',', '"', '\n', '\r']) {
                    format!("\"{}\"", c.replace('"', "\"\""))
                } else {
                    c.clone()
                }
            })
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_quotes_and_newlines() {
        let rows = vec![
            vec!["a".into(), "b,c".into(), "d\"e".into()],
            vec!["línea1\nlínea2".into(), "".into(), "ñ".into()],
        ];
        assert_eq!(parse(&write(&rows)), rows);
    }

    #[test]
    fn handles_crlf_and_bom() {
        assert_eq!(parse("\u{feff}x,y\r\n1,2\r\n"), vec![vec!["x", "y"], vec!["1", "2"]]);
    }
}
