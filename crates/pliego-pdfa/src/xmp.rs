//! Metadatos XMP de PDF/A a partir del diccionario Info (los dos deben coincidir).

use std::time::{SystemTime, UNIX_EPOCH};

use crate::Level;

/// Valores del diccionario Info que se reflejan en XMP.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct InfoValues {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    /// Fecha de creación en formato PDF (`D:AAAAMMDDhhmmss...`).
    pub created: Option<String>,
    pub modified: Option<String>,
}

pub(crate) fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // caracteres de control no permitidos en XML 1.0
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

/// `D:20260102030405+01'00'` → `2026-01-02T03:04:05+01:00`. Devuelve `None` si no se entiende.
pub(crate) fn pdf_date_to_xmp(d: &str) -> Option<String> {
    let d = d.strip_prefix("D:").unwrap_or(d);
    let digits = |from: usize, len: usize| -> Option<&str> {
        let part = d.get(from..from + len)?;
        part.chars().all(|c| c.is_ascii_digit()).then_some(part)
    };
    let year = digits(0, 4)?;
    let month = digits(4, 2).unwrap_or("01");
    let day = digits(6, 2).unwrap_or("01");
    let (h, m, s) = (
        digits(8, 2).unwrap_or("00"),
        digits(10, 2).unwrap_or("00"),
        digits(12, 2).unwrap_or("00"),
    );
    let rest = d.get(14..).unwrap_or("");
    let tz = match rest.chars().next() {
        Some(sign @ ('+' | '-')) => {
            let t: String = rest[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            let hh = t.get(0..2).unwrap_or("00");
            let mm = t.get(2..4).unwrap_or("00");
            format!("{sign}{hh}:{mm}")
        }
        _ => "Z".to_string(),
    };
    Some(format!("{year}-{month}-{day}T{h}:{m}:{s}{tz}"))
}

/// Fecha y hora UTC actuales en formato PDF.
pub(crate) fn now_pdf_date() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Algoritmo de días civiles (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "D:{y:04}{m:02}{d:02}{:02}{:02}{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn alt(tag: &str, v: &str) -> String {
    format!(
        "<{tag}><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></{tag}>",
        escape(v)
    )
}

/// Paquete XMP con la identificación PDF/A y los valores de Info.
pub(crate) fn build_xmp(level: Level, info: &InfoValues) -> String {
    let mut dc = String::new();
    if let Some(t) = &info.title {
        dc += &alt("dc:title", t);
    }
    if let Some(a) = &info.author {
        dc += &format!(
            "<dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>",
            escape(a)
        );
    }
    if let Some(s) = &info.subject {
        dc += &alt("dc:description", s);
    }
    let mut pdf = String::new();
    if let Some(k) = &info.keywords {
        pdf += &format!("<pdf:Keywords>{}</pdf:Keywords>", escape(k));
    }
    if let Some(p) = &info.producer {
        pdf += &format!("<pdf:Producer>{}</pdf:Producer>", escape(p));
    }
    let mut xmp = String::new();
    if let Some(c) = &info.creator {
        xmp += &format!("<xmp:CreatorTool>{}</xmp:CreatorTool>", escape(c));
    }
    if let Some(d) = info.created.as_deref().and_then(pdf_date_to_xmp) {
        xmp += &format!("<xmp:CreateDate>{d}</xmp:CreateDate>");
    }
    if let Some(d) = info.modified.as_deref().and_then(pdf_date_to_xmp) {
        xmp += &format!("<xmp:ModifyDate>{d}</xmp:ModifyDate>");
    }
    let mut body = format!(
        "<rdf:Description rdf:about=\"\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\
         <pdfaid:part>{}</pdfaid:part><pdfaid:conformance>{}</pdfaid:conformance></rdf:Description>",
        level.part(),
        level.conformance()
    );
    if !dc.is_empty() {
        body += &format!(
            "<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">{dc}</rdf:Description>"
        );
    }
    if !pdf.is_empty() {
        body += &format!(
            "<rdf:Description rdf:about=\"\" xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\">{pdf}</rdf:Description>"
        );
    }
    if !xmp.is_empty() {
        body += &format!(
            "<rdf:Description rdf:about=\"\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">{xmp}</rdf:Description>"
        );
    }
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
         {body}</rdf:RDF></x:xmpmeta>\n<?xpacket end=\"w\"?>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(
            pdf_date_to_xmp("D:20260102030405+01'00'").unwrap(),
            "2026-01-02T03:04:05+01:00"
        );
        assert_eq!(pdf_date_to_xmp("D:20260102030405Z").unwrap(), "2026-01-02T03:04:05Z");
        assert_eq!(pdf_date_to_xmp("D:2026").unwrap(), "2026-01-01T00:00:00Z");
        assert!(pdf_date_to_xmp("ayer").is_none());
        let now = now_pdf_date();
        assert!(
            now.starts_with("D:20") && now.ends_with('Z') && now.len() == 17,
            "{now}"
        );
    }

    #[test]
    fn xmp_escapes_and_identifies() {
        let info = InfoValues {
            title: Some("A & <B>".into()),
            author: Some("Ñ".into()),
            ..Default::default()
        };
        let x = build_xmp(Level::A3b, &info);
        assert!(x.contains("A &amp; &lt;B&gt;") && x.contains("<pdfaid:part>3</pdfaid:part>"));
        assert_eq!(
            crate::validate::xmp_value(&x, "pdfaid:conformance").as_deref(),
            Some("B")
        );
    }
}
