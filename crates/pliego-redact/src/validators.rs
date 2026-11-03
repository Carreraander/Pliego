//! Validadores de identificadores españoles y bancarios. Reciben el texto tal como aparece
//! (con o sin espacios y guiones) y comprueban estructura y dígito o letra de control.

const DNI_LETTERS: &[u8; 23] = b"TRWAGMYFPDXBNJZSQVHLCKE";

fn compact(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '.')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn dni_letter(number: u32) -> char {
    DNI_LETTERS[(number % 23) as usize] as char
}

/// DNI: 8 cifras y letra de control.
pub fn is_valid_dni(s: &str) -> bool {
    let s = compact(s);
    let b = s.as_bytes();
    if b.len() != 9 || !b[..8].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let n: u32 = s[..8].parse().unwrap_or(0);
    dni_letter(n) == b[8] as char
}

/// NIE: X, Y o Z, 7 cifras y letra de control.
pub fn is_valid_nie(s: &str) -> bool {
    let s = compact(s);
    let b = s.as_bytes();
    if b.len() != 9 || !b[1..8].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let prefix = match b[0] {
        b'X' => '0',
        b'Y' => '1',
        b'Z' => '2',
        _ => return false,
    };
    let n: u32 = format!("{prefix}{}", &s[1..8]).parse().unwrap_or(0);
    dni_letter(n) == b[8] as char
}

/// CIF: letra de tipo, 7 cifras y control (cifra o letra según el tipo).
pub fn is_valid_cif(s: &str) -> bool {
    let s = compact(s);
    let b = s.as_bytes();
    if b.len() != 9 || !b"ABCDEFGHJNPQRSUVW".contains(&b[0]) || !b[1..8].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let digits: Vec<u32> = b[1..8].iter().map(|c| (c - b'0') as u32).collect();
    let mut sum = 0;
    for (i, d) in digits.iter().enumerate() {
        if i % 2 == 0 {
            let x = d * 2;
            sum += x / 10 + x % 10;
        } else {
            sum += d;
        }
    }
    let control = (10 - sum % 10) % 10;
    let letter = b"JABCDEFGHI"[control as usize];
    let digit = b'0' + control as u8;
    match b[0] {
        // Solo letra.
        b'N' | b'P' | b'Q' | b'R' | b'S' | b'W' => b[8] == letter,
        // Solo cifra.
        b'A' | b'B' | b'E' | b'H' => b[8] == digit,
        _ => b[8] == digit || b[8] == letter,
    }
}

/// IBAN: estructura y dígitos de control (módulo 97). Para ES exige además 24 caracteres.
pub fn is_valid_iban(s: &str) -> bool {
    let s = compact(s);
    let b = s.as_bytes();
    if b.len() < 15
        || b.len() > 34
        || !b[..2].iter().all(u8::is_ascii_uppercase)
        || !b[2..4].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    if !b.iter().all(u8::is_ascii_alphanumeric) {
        return false;
    }
    if s.starts_with("ES") && b.len() != 24 {
        return false;
    }
    let rotated = s[4..].chars().chain(s[..4].chars());
    let mut rem: u32 = 0;
    for c in rotated {
        let v = match c {
            '0'..='9' => c as u32 - '0' as u32,
            'A'..='Z' => c as u32 - 'A' as u32 + 10,
            _ => return false,
        };
        rem = if v >= 10 {
            (rem * 100 + v) % 97
        } else {
            (rem * 10 + v) % 97
        };
    }
    rem == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dni() {
        assert!(is_valid_dni("12345678Z"));
        assert!(is_valid_dni("12.345.678-z"));
        assert!(is_valid_dni("00000000T"));
        assert!(!is_valid_dni("12345678A"));
        assert!(!is_valid_dni("1234567Z"));
        assert!(!is_valid_dni("123456789"));
    }

    #[test]
    fn nie() {
        assert!(is_valid_nie("X1234567L"));
        assert!(is_valid_nie("x-1234567-l"));
        assert!(is_valid_nie("Y0000000Z"));
        assert!(!is_valid_nie("X1234567A"));
        assert!(!is_valid_nie("A1234567L"));
    }

    #[test]
    fn cif() {
        assert!(is_valid_cif("A58818501")); // cifra
        assert!(is_valid_cif("Q2826000H")); // letra (organismo público)
        assert!(is_valid_cif("B12345674"));
        assert!(!is_valid_cif("A58818502"));
        assert!(!is_valid_cif("A5881850J")); // tipo A exige cifra
        assert!(!is_valid_cif("Q28260001")); // tipo Q exige letra
        assert!(!is_valid_cif("I58818501"));
    }

    #[test]
    fn iban() {
        assert!(is_valid_iban("ES91 2100 0418 4502 0005 1332"));
        assert!(is_valid_iban("es9121000418450200051332"));
        assert!(is_valid_iban("GB82 WEST 1234 5698 7654 32"));
        assert!(is_valid_iban("DE89 3704 0044 0532 0130 00"));
        assert!(!is_valid_iban("ES91 2100 0418 4502 0005 1333"));
        assert!(!is_valid_iban("ES91 2100 0418 4502 0005 133"));
        assert!(!is_valid_iban("1234"));
    }
}
