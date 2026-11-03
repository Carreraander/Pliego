//! Análisis de los cambios hechos después de una firma: ¿son solo de los que PAdES permite?

use lopdf::{Dictionary, Object};

use crate::sign::last_startxref;

fn without(d: &Dictionary, skip: &[&[u8]]) -> Vec<(Vec<u8>, Object)> {
    d.iter()
        .filter(|(k, _)| !skip.contains(&k.as_slice()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn same_except(a: &Dictionary, b: &Dictionary, skip: &[&[u8]]) -> bool {
    let (mut x, mut y) = (without(a, skip), without(b, skip));
    x.sort_by(|p, q| p.0.cmp(&q.0));
    y.sort_by(|p, q| p.0.cmp(&q.0));
    x == y
}

/// Comprueba que lo añadido después de `signed_len` bytes solo contiene cambios permitidos:
/// objetos nuevos, el DSS en el catálogo, campos de firma nuevos y las listas que los referencian.
pub fn check_updates(data: &[u8], signed_len: usize, mdp: Option<u8>) -> Result<(), String> {
    // DocMDP 1: no se permite ningún cambio salvo el DSS (mantenimiento de la firma).
    let frozen = mdp == Some(1);
    let before =
        lopdf::Document::load_mem(&data[..signed_len]).map_err(|e| format!("revisión firmada ilegible: {e}"))?;
    let after =
        lopdf::Document::load_mem(data).map_err(|e| format!("el documento tiene datos añadidos ilegibles: {e}"))?;
    let (Ok((p0, _)), Ok((p1, _))) = (last_startxref(&data[..signed_len]), last_startxref(data)) else {
        return Err("no se encuentra la tabla de referencias".into());
    };
    if p0 == p1 {
        return Err("hay datos añadidos que no forman una actualización del documento".into());
    }
    let root_id = after.trailer.get(b"Root").and_then(Object::as_reference).ok();
    for (id, new) in &after.objects {
        let Some(old) = before.objects.get(id) else { continue };
        if old == new {
            continue;
        }
        let ok = match (old, new) {
            (Object::Dictionary(a), Object::Dictionary(b)) if Some(*id) == root_id => same_except(
                a,
                b,
                if frozen {
                    &[b"DSS", b"Extensions"]
                } else {
                    &[b"DSS", b"AcroForm", b"Extensions"]
                },
            ),
            (Object::Dictionary(a), Object::Dictionary(b))
                if a.get(b"Type").ok() == Some(&Object::Name(b"Page".to_vec())) =>
            {
                !frozen && same_except(a, b, &[b"Annots"])
            }
            (Object::Dictionary(a), Object::Dictionary(b)) if a.has(b"Fields") => {
                !frozen && same_except(a, b, &[b"Fields", b"SigFlags"])
            }
            // Listas de anotaciones o de campos que solo crecen.
            (Object::Array(a), Object::Array(b)) => !frozen && b.len() >= a.len() && b[..a.len()] == a[..],
            _ => false,
        };
        if !ok {
            return Err(format!("se modificó el objeto {} {} después de la firma", id.0, id.1));
        }
    }
    Ok(())
}
