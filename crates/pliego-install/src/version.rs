use std::cmp::Ordering;

/// Una versión es un nombre de carpeta seguro: letras, dígitos, `.`, `-` y `+`, sin empezar por `.`.
pub fn is_valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 64
        && !v.starts_with('.')
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

/// Compara versiones tipo `1.2.3` numéricamente por componente; una versión con sufijo `-pre` es menor que sin él.
pub fn cmp_versions(a: &str, b: &str) -> Ordering {
    fn split(v: &str) -> (Vec<u64>, bool) {
        let core = v.split('+').next().unwrap_or(v);
        let (nums, pre) = match core.split_once('-') {
            Some((n, _)) => (n, true),
            None => (core, false),
        };
        (nums.split('.').map(|p| p.parse().unwrap_or(0)).collect(), pre)
    }
    let (na, pa) = split(a);
    let (nb, pb) = split(b);
    na.cmp(&nb).then(pb.cmp(&pa)).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_names() {
        assert!(is_valid_version("0.4.0"));
        assert!(is_valid_version("1.0.0-beta.1"));
        for bad in ["", ".", "..", "../x", "a/b", "a\\b", ".hidden", "a b", "C:x"] {
            assert!(!is_valid_version(bad), "{bad}");
        }
    }

    #[test]
    fn orders_numerically() {
        assert_eq!(cmp_versions("0.10.0", "0.9.0"), Ordering::Greater);
        assert_eq!(cmp_versions("1.0.0-beta", "1.0.0"), Ordering::Less);
        assert_eq!(cmp_versions("1.2.3", "1.2.3"), Ordering::Equal);
    }
}
