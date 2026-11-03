//! Edición de la variable PATH (texto), sin tocar el registro: así se puede probar en cualquier sistema.

fn same(a: &str, b: &str) -> bool {
    a.trim_end_matches('\\').eq_ignore_ascii_case(b.trim_end_matches('\\'))
}

/// `Some(nuevo)` si hay que añadir `dir`; `None` si ya estaba.
pub fn with_dir(existing: &str, dir: &str) -> Option<String> {
    if existing.split(';').any(|p| same(p.trim(), dir)) {
        return None;
    }
    let base = existing.trim_end_matches(';');
    Some(if base.is_empty() {
        dir.to_string()
    } else {
        format!("{base};{dir}")
    })
}

/// `Some(nuevo)` si `dir` estaba y se quita; `None` si no estaba.
pub fn without_dir(existing: &str, dir: &str) -> Option<String> {
    let parts: Vec<&str> = existing.split(';').collect();
    if !parts.iter().any(|p| same(p.trim(), dir)) {
        return None;
    }
    Some(
        parts
            .into_iter()
            .filter(|p| !same(p.trim(), dir))
            .collect::<Vec<_>>()
            .join(";"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_once_ignoring_case_and_trailing_slash() {
        assert_eq!(with_dir("C:\\a;C:\\b", "C:\\P\\bin").unwrap(), "C:\\a;C:\\b;C:\\P\\bin");
        assert_eq!(with_dir("", "C:\\P\\bin").unwrap(), "C:\\P\\bin");
        assert_eq!(with_dir("C:\\a;", "C:\\P\\bin").unwrap(), "C:\\a;C:\\P\\bin");
        assert_eq!(with_dir("C:\\a;c:\\p\\BIN\\", "C:\\P\\bin"), None);
    }

    #[test]
    fn removes_only_that_entry() {
        assert_eq!(
            without_dir("C:\\a;C:\\P\\bin;C:\\b", "C:\\P\\bin").unwrap(),
            "C:\\a;C:\\b"
        );
        assert_eq!(without_dir("C:\\P\\bin", "C:\\P\\bin").unwrap(), "");
        assert_eq!(without_dir("C:\\a", "C:\\P\\bin"), None);
        assert_eq!(without_dir("C:\\a;%USERPROFILE%\\bin", "C:\\P\\bin"), None);
    }
}
