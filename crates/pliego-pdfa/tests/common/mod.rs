#![allow(dead_code)]
//! Constructor mínimo de PDFs para pruebas, con variantes que incumplen una regla cada una.

use pliego_pdfa::srgb_profile;

#[derive(Clone, Default)]
pub struct Opts {
    pub xmp: Option<String>,
    pub no_xmp: bool,
    pub no_id: bool,
    pub no_intent: bool,
    pub helvetica_not_embedded: bool,
    pub javascript: bool,
    pub transparency: bool,
    pub embedded_file: bool,
    pub af: bool,
    pub encrypted_marker: bool,
    pub hidden_annot: bool,
    pub lzw: bool,
    pub ocg: bool,
    pub type0_without_tounicode: bool,
}

pub fn xmp(part: u8, conf: &str) -> String {
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
         <rdf:Description rdf:about=\"\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\
         <pdfaid:part>{part}</pdfaid:part><pdfaid:conformance>{conf}</pdfaid:conformance>\
         </rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>"
    )
}

struct B {
    objs: Vec<Vec<u8>>,
}

impl B {
    fn add(&mut self, body: impl Into<Vec<u8>>) -> usize {
        self.objs.push(body.into());
        self.objs.len()
    }
    fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
        let mut v = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        v.extend(data);
        v.extend(b"\nendstream");
        v
    }
}

/// Documento de una página con texto en fuente Type3 (siempre «incrustada»), conforme por defecto.
pub fn build(o: &Opts, part: u8, conf: &str) -> Vec<u8> {
    let mut b = B { objs: Vec::new() };
    let charproc = b.add(B::stream("", b"500 0 0 0 500 700 d1 0 0 500 700 re f"));
    let type3 = b.add(format!(
        "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 500 700] /FontMatrix [0.001 0 0 0.001 0 0] \
         /CharProcs << /a {charproc} 0 R >> /Encoding << /Type /Encoding /Differences [97 /a] >> \
         /FirstChar 97 /LastChar 97 /Widths [500] >>"
    ));
    let helv = b.add(if o.helvetica_not_embedded {
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string()
    } else {
        format!("<< /Type /Font /Subtype /Type3 /FontBBox [0 0 500 700] /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << /a {charproc} 0 R >> /Encoding << /Type /Encoding /Differences [97 /a] >> /FirstChar 97 /LastChar 97 /Widths [500] >>")
    });
    let type0 = if o.type0_without_tounicode {
        Some(b.add("<< /Type /Font /Subtype /Type0 /BaseFont /X /Encoding /Identity-H /DescendantFonts [] >>"))
    } else {
        None
    };
    let gs = b.add(if o.transparency {
        "<< /Type /ExtGState /ca 0.5 >>"
    } else {
        "<< /Type /ExtGState /ca 1 >>"
    });
    let content = if o.lzw {
        b.add(B::stream("/Filter /LZWDecode", b"\x80\x0b\x60\x50\x22\x0c\x0c\x85\x01"))
    } else {
        b.add(B::stream(
            "",
            b"/GS gs BT /F1 24 Tf 72 700 Td (aaa) Tj /F2 12 Tf (a) Tj ET",
        ))
    };
    let annot = if o.hidden_annot {
        Some(b.add("<< /Type /Annot /Subtype /Text /Rect [10 10 30 30] /Contents (x) /F 2 >>"))
    } else {
        None
    };
    let pages_id = b.objs.len() + 2;
    let font2 = type0.map(|t| format!(" /F3 {t} 0 R")).unwrap_or_default();
    let annots = annot.map(|a| format!(" /Annots [{a} 0 R]")).unwrap_or_default();
    let page = b.add(format!(
        "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Contents {content} 0 R \
         /Resources << /Font << /F1 {type3} 0 R /F2 {helv} 0 R{font2} >> /ExtGState << /GS {gs} 0 R >> >>{annots} >>"
    ));
    assert_eq!(
        b.add(format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>")),
        pages_id
    );

    let mut catalog = format!("<< /Type /Catalog /Pages {pages_id} 0 R");
    if !o.no_xmp {
        let x = o.xmp.clone().unwrap_or_else(|| xmp(part, conf));
        let m = b.add(B::stream("/Type /Metadata /Subtype /XML", x.as_bytes()));
        catalog += &format!(" /Metadata {m} 0 R");
    }
    if !o.no_intent {
        let icc = b.add(B::stream("/N 3", &srgb_profile()));
        let oi = b.add(format!(
            "<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier (sRGB) /DestOutputProfile {icc} 0 R >>"
        ));
        catalog += &format!(" /OutputIntents [{oi} 0 R]");
    }
    if o.javascript {
        let js = b.add("<< /S /JavaScript /JS (app.alert\\(1\\)) >>");
        catalog += &format!(" /OpenAction {js} 0 R");
    }
    if o.ocg {
        let g = b.add("<< /Type /OCG /Name (capa) >>");
        catalog += &format!(" /OCProperties << /OCGs [{g} 0 R] /D << /Order [{g} 0 R] >> >>");
    }
    if o.embedded_file {
        let data = b.add(B::stream("/Type /EmbeddedFile", b"datos"));
        let fs = b.add(format!(
            "<< /Type /Filespec /F (a.txt) /UF (a.txt) /AFRelationship /Data /EF << /F {data} 0 R >> >>"
        ));
        catalog += &format!(" /Names << /EmbeddedFiles << /Names [(a.txt) {fs} 0 R] >> >>");
        if o.af {
            catalog += &format!(" /AF [{fs} 0 R]");
        }
    }
    catalog += " >>";
    let cat = b.add(catalog);

    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offs = Vec::new();
    for (i, body) in b.objs.iter().enumerate() {
        offs.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", b.objs.len() + 1).bytes());
    for off in offs {
        out.extend(format!("{off:010} 00000 n \n").bytes());
    }
    let _ = o.encrypted_marker;
    let id = if o.no_id {
        ""
    } else {
        " /ID [<0123456789ABCDEF0123456789ABCDEF> <0123456789ABCDEF0123456789ABCDEF>]"
    };
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root {cat} 0 R{id} >>\nstartxref\n{xref}\n%%EOF\n",
            b.objs.len() + 1
        )
        .bytes(),
    );
    out
}

pub fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}
