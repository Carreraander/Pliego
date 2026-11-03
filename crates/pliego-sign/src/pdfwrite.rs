//! Escritura mínima de objetos PDF para las actualizaciones incrementales.

use lopdf::{Dictionary, Object, StringFormat};

pub fn write_name(name: &[u8], out: &mut Vec<u8>) {
    out.push(b'/');
    for &b in name {
        if b.is_ascii_alphanumeric() || b"-_.+".contains(&b) {
            out.push(b);
        } else {
            out.extend_from_slice(format!("#{b:02X}").as_bytes());
        }
    }
}

pub fn write_string(bytes: &[u8], format: StringFormat, out: &mut Vec<u8>) {
    match format {
        StringFormat::Hexadecimal => {
            out.push(b'<');
            out.extend_from_slice(hex::encode_upper(bytes).as_bytes());
            out.push(b'>');
        }
        StringFormat::Literal => {
            out.push(b'(');
            for &b in bytes {
                if matches!(b, b'(' | b')' | b'\\') {
                    out.push(b'\\');
                }
                out.push(b);
            }
            out.push(b')');
        }
    }
}

/// Cadena de texto PDF: ASCII literal o UTF-16BE con BOM en hexadecimal.
pub fn text_string(s: &str, out: &mut Vec<u8>) {
    if s.is_ascii() && !s.contains(['\r', '\n']) {
        write_string(s.as_bytes(), StringFormat::Literal, out);
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        write_string(&bytes, StringFormat::Hexadecimal, out);
    }
}

pub fn write_dict(d: &Dictionary, out: &mut Vec<u8>) {
    out.extend_from_slice(b"<<");
    for (k, v) in d.iter() {
        write_name(k, out);
        out.push(b' ');
        write_object(v, out);
        out.push(b' ');
    }
    out.extend_from_slice(b">>");
}

/// Serializa un objeto que no sea flujo (los flujos nunca se reescriben en una actualización incremental).
pub fn write_object(o: &Object, out: &mut Vec<u8>) {
    match o {
        Object::Null | Object::Stream(_) => out.extend_from_slice(b"null"),
        Object::Boolean(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Object::Integer(i) => out.extend_from_slice(i.to_string().as_bytes()),
        Object::Real(r) => out.extend_from_slice(format!("{r}").as_bytes()),
        Object::Name(n) => write_name(n, out),
        Object::String(s, f) => write_string(s, *f, out),
        Object::Array(a) => {
            out.push(b'[');
            for (i, v) in a.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                write_object(v, out);
            }
            out.push(b']');
        }
        Object::Dictionary(d) => write_dict(d, out),
        Object::Reference((n, g)) => out.extend_from_slice(format!("{n} {g} R").as_bytes()),
    }
}
