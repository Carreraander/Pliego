//! Escritor de XLSX (SpreadsheetML): una hoja por tabla o por página.

use std::fmt::Write as _;

use pliego_core::Result;

use crate::model::*;
use crate::util::{Package, xml_escape};

/// Interpreta un texto como número (admite `1.234,56`, `1,234.56`, `12 %`, `45 €`). `None` si no lo es.
pub(crate) fn parse_number(text: &str) -> Option<f64> {
    let mut t: String = text.trim().replace(['\u{a0}', ' '], "");
    for sym in ['€', '$', '£', '%'] {
        t = t.replace(sym, "");
    }
    if t.is_empty() || !t.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let (neg, body) = match t.strip_prefix('-').or(t.strip_prefix('−')) {
        Some(b) => (true, b.to_string()),
        None => (false, t.trim_start_matches('+').to_string()),
    };
    if !body.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
        return None;
    }
    // Ceros a la izquierda ("007"): es un código, no un número.
    if body.len() > 1 && body.starts_with('0') && !body.starts_with("0.") && !body.starts_with("0,") {
        return None;
    }
    let (dots, commas) = (body.matches('.').count(), body.matches(',').count());
    let thousands = |s: &str, sep: char| {
        let parts: Vec<&str> = s.split(sep).collect();
        parts[0].len() <= 3 && !parts[0].is_empty() && parts[1..].iter().all(|p| p.len() == 3)
    };
    let normalized = if dots > 0 && commas > 0 {
        // El último separador es el decimal.
        let last_dot = body.rfind('.')?;
        let last_comma = body.rfind(',')?;
        if last_comma > last_dot {
            body.replace('.', "").replace(',', ".")
        } else {
            body.replace(',', "")
        }
    } else if commas > 0 {
        if commas > 1 || thousands(&body, ',') {
            body.replace(',', "")
        } else {
            body.replace(',', ".")
        }
    } else if dots > 0 {
        if dots > 1 || thousands(&body, '.') {
            body.replace('.', "")
        } else {
            body
        }
    } else {
        body
    };
    let v: f64 = normalized.parse().ok()?;
    Some(if neg { -v } else { v })
}

fn col_name(mut n: usize) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s
}

fn sheet_name(base: &str, used: &mut Vec<String>) -> String {
    let clean: String = base.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
    let mut name = clean.clone();
    let mut n = 2;
    while used.iter().any(|u| u.eq_ignore_ascii_case(&name)) {
        let suffix = format!(" ({n})");
        name = format!("{}{suffix}", clean.chars().take(31 - suffix.len()).collect::<String>());
        n += 1;
    }
    used.push(name.clone());
    name
}

/// Celda: texto, negrita y alineación.
type SheetCell = Option<(String, bool, Align)>;

struct Sheet {
    name: String,
    /// Anchos de columna en caracteres.
    widths: Vec<f32>,
    /// (texto, negrita, alineación) por celda; `span` se ignora (la celda va en su primera columna).
    rows: Vec<Vec<SheetCell>>,
}

fn table_sheet(t: &Table, name: String) -> Sheet {
    let widths = t.cols.iter().map(|(l, r)| ((r - l) / 5.0).clamp(6.0, 80.0)).collect();
    let mut rows = Vec::new();
    for row in &t.rows {
        let mut out: Vec<SheetCell> = Vec::new();
        for cell in row {
            let bold = !cell.runs.is_empty() && cell.runs.iter().all(|r| r.bold || r.text.trim().is_empty());
            out.push((!cell.is_empty()).then(|| (cell.text(), bold, cell.align)));
            for _ in 1..cell.span {
                out.push(None);
            }
        }
        rows.push(out);
    }
    Sheet { name, widths, rows }
}

/// Hoja con el texto de una página cuando no hay tablas: un párrafo por fila.
fn text_sheet(page: &PageLayout, name: String) -> Sheet {
    let mut rows = Vec::new();
    for el in page.elements() {
        if let Element::Paragraph(p) = el {
            rows.push(vec![Some((p.text().replace('\t', " "), false, Align::Left))]);
        }
    }
    Sheet {
        name,
        widths: vec![100.0],
        rows,
    }
}

/// Hojas según el modo: `per_page` junta todas las tablas de una página en una hoja, una debajo de otra.
pub(crate) type SheetData = (String, Vec<f32>, Vec<Vec<SheetCell>>);

pub(crate) fn build_sheets(layout: &Layout, per_page: bool) -> (Vec<SheetData>, bool) {
    let mut used = Vec::new();
    let mut sheets: Vec<Sheet> = Vec::new();
    let mut any_table = false;
    let multi_page = layout.pages.len() > 1;
    for page in &layout.pages {
        let tables: Vec<&Table> = page.tables().collect();
        let prefix = format!("{} {}", layout.lang.pick("Pág", "Page"), page.index + 1);
        if tables.is_empty() {
            continue;
        }
        any_table = true;
        if per_page {
            let mut merged = table_sheet(tables[0], sheet_name(&prefix, &mut used));
            for t in &tables[1..] {
                merged.rows.push(Vec::new());
                let extra = table_sheet(t, String::new());
                if extra.widths.len() > merged.widths.len() {
                    merged.widths = extra.widths.clone();
                }
                merged.rows.extend(extra.rows);
            }
            sheets.push(merged);
        } else {
            for (i, t) in tables.iter().enumerate() {
                let base = match (multi_page, tables.len()) {
                    (false, 1) => layout.lang.pick("Tabla", "Table").to_string(),
                    (false, _) => format!("{} {}", layout.lang.pick("Tabla", "Table"), i + 1),
                    (true, 1) => prefix.clone(),
                    (true, _) => format!("{prefix} - {} {}", layout.lang.pick("Tabla", "Table"), i + 1),
                };
                sheets.push(table_sheet(t, sheet_name(&base, &mut used)));
            }
        }
    }
    if !any_table {
        for page in &layout.pages {
            let name = sheet_name(
                &format!("{} {}", layout.lang.pick("Pág", "Page"), page.index + 1),
                &mut used,
            );
            sheets.push(text_sheet(page, name));
        }
    }
    (
        sheets.into_iter().map(|s| (s.name, s.widths, s.rows)).collect(),
        any_table,
    )
}

fn sheet_xml(widths: &[f32], rows: &[Vec<SheetCell>]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><cols>",
    );
    for (i, w) in widths.iter().enumerate() {
        let _ = write!(
            s,
            "<col min=\"{}\" max=\"{}\" width=\"{w:.1}\" customWidth=\"1\"/>",
            i + 1,
            i + 1
        );
    }
    s.push_str("</cols><sheetData>");
    for (r, row) in rows.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        let _ = write!(s, "<row r=\"{}\">", r + 1);
        for (c, cell) in row.iter().enumerate() {
            let Some((text, bold, align)) = cell else { continue };
            let style = match (bold, align) {
                (false, Align::Right) => 1,
                (true, Align::Right) => 3,
                (true, _) => 2,
                _ => 0,
            };
            let at = format!("{}{}", col_name(c), r + 1);
            match parse_number(text) {
                Some(v) => {
                    let _ = write!(s, "<c r=\"{at}\" s=\"{style}\"><v>{v}</v></c>");
                }
                None => {
                    let _ = write!(
                        s,
                        "<c r=\"{at}\" s=\"{style}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                        xml_escape(text)
                    );
                }
            }
        }
        s.push_str("</row>");
    }
    s.push_str("</sheetData></worksheet>");
    s
}

const STYLES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\
<fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font><font><b/><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>\
<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills>\
<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>\
<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>\
<cellXfs count=\"4\">\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\
<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyAlignment=\"1\"><alignment horizontal=\"right\"/></xf>\
<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/>\
<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\" applyAlignment=\"1\"><alignment horizontal=\"right\"/></xf>\
</cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>";

pub(crate) fn write_xlsx(layout: &Layout, per_page: bool) -> Result<(Vec<u8>, usize, bool)> {
    let (sheets, any_table) = build_sheets(layout, per_page);
    let main = "application/vnd.openxmlformats-officedocument.spreadsheetml";
    let mut ct = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Override PartName=\"/xl/workbook.xml\" ContentType=\"{main}.sheet.main+xml\"/>\
         <Override PartName=\"/xl/styles.xml\" ContentType=\"{main}.styles+xml\"/>"
    );
    let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let mut wb = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>",
    );
    let mut rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rIdSty\" Type=\"{rel}/styles\" Target=\"styles.xml\"/>"
    );
    let mut pkg = Package::new();
    let n = sheets.len().max(1);
    let sheets = if sheets.is_empty() {
        vec![(
            format!("{} 1", layout.lang.pick("Hoja", "Sheet")),
            vec![10.0],
            Vec::new(),
        )]
    } else {
        sheets
    };
    for (i, (name, widths, rows)) in sheets.iter().enumerate() {
        let k = i + 1;
        let _ = write!(
            ct,
            "<Override PartName=\"/xl/worksheets/sheet{k}.xml\" ContentType=\"{main}.worksheet+xml\"/>"
        );
        let _ = write!(
            wb,
            "<sheet name=\"{}\" sheetId=\"{k}\" r:id=\"rIdS{k}\"/>",
            xml_escape(name)
        );
        let _ = write!(
            rels,
            "<Relationship Id=\"rIdS{k}\" Type=\"{rel}/worksheet\" Target=\"worksheets/sheet{k}.xml\"/>"
        );
        pkg.add(
            &format!("xl/worksheets/sheet{k}.xml"),
            sheet_xml(widths, rows).as_bytes(),
        )?;
    }
    ct.push_str("</Types>");
    wb.push_str("</sheets></workbook>");
    rels.push_str("</Relationships>");
    pkg.add("[Content_Types].xml", ct.as_bytes())?;
    pkg.add(
        "_rels/.rels",
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Id=\"rId1\" Type=\"{rel}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"
        )
        .as_bytes(),
    )?;
    pkg.add("xl/workbook.xml", wb.as_bytes())?;
    pkg.add("xl/_rels/workbook.xml.rels", rels.as_bytes())?;
    pkg.add("xl/styles.xml", STYLES.as_bytes())?;
    Ok((pkg.finish()?, n, any_table))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(parse_number("1.234,56"), Some(1234.56));
        assert_eq!(parse_number("1,234.56"), Some(1234.56));
        assert_eq!(parse_number("12,50 €"), Some(12.5));
        assert_eq!(parse_number("3.287,90"), Some(3287.9));
        assert_eq!(parse_number("86.092"), Some(86092.0));
        assert_eq!(parse_number("-5"), Some(-5.0));
        assert_eq!(parse_number("007"), None);
        assert_eq!(parse_number("FA-2026"), None);
        assert_eq!(parse_number("Total"), None);
        assert_eq!(parse_number("21"), Some(21.0));
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(26), "AA");
    }
}
