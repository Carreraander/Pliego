//! Formato y análisis de números, fechas y textos con la sintaxis de los formularios de Acrobat
//! (`util.printf`, `util.printd`, `util.scand`, `AFNumber_Format`...).
//!
//! Todo son funciones puras: el intérprete las llama a través de `__h(..)` y se prueban sin JavaScript.

use std::fmt::Write as _;

/// Idioma de los nombres de meses, de días y de los mensajes de validación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Es,
    En,
}

impl Language {
    pub fn months(self) -> [&'static str; 12] {
        match self {
            Language::Es => [
                "enero",
                "febrero",
                "marzo",
                "abril",
                "mayo",
                "junio",
                "julio",
                "agosto",
                "septiembre",
                "octubre",
                "noviembre",
                "diciembre",
            ],
            Language::En => [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ],
        }
    }

    pub fn months_short(self) -> [&'static str; 12] {
        match self {
            Language::Es => [
                "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
            ],
            Language::En => [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ],
        }
    }

    /// Lunes a domingo no: el índice 0 es el domingo, como en JavaScript.
    pub fn days(self) -> [&'static str; 7] {
        match self {
            Language::Es => ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
            Language::En => [
                "Sunday",
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
            ],
        }
    }

    pub fn days_short(self) -> [&'static str; 7] {
        match self {
            Language::Es => ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"],
            Language::En => ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        }
    }
}

/// Fecha y hora sin zona horaria. El mes va de 0 a 11 (como `Date` de JavaScript).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateParts {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

pub fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i32, m0: u32) -> u32 {
    match m0 {
        0 | 2 | 4 | 6 | 7 | 9 | 11 => 31,
        3 | 5 | 8 | 10 => 30,
        _ if is_leap(y) => 29,
        _ => 28,
    }
}

/// Días desde 1970-01-01 (algoritmo civil de Howard Hinnant, dominio público).
fn days_from_civil(y: i32, m1: u32, d: u32) -> i64 {
    let y = i64::from(if m1 <= 2 { y - 1 } else { y });
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((m1 + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Día de la semana (0 = domingo).
pub fn weekday(y: i32, m0: u32, d: u32) -> u32 {
    (days_from_civil(y, m0 + 1, d) + 4).rem_euclid(7) as u32
}

impl DateParts {
    pub fn is_valid(&self) -> bool {
        self.month < 12
            && self.day >= 1
            && self.day <= days_in_month(self.year, self.month)
            && self.hour < 24
            && self.minute < 60
            && self.second < 60
    }
}

/// Fecha de hoy según el reloj del sistema (UTC), o `(año, mes 0-11, día)`.
pub fn today(now_ms: Option<f64>) -> (i32, u32, u32) {
    let ms = now_ms.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_millis() as f64)
    });
    let days = (ms / 86_400_000.0).floor() as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m1 = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400) as i32 + i32::from(m1 <= 2);
    (y, m1 - 1, d)
}

// --- Patrones de fecha -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    /// Letra de patrón repetida `n` veces.
    Field(char, usize),
    Lit(String),
}

fn tokenize_pattern(pat: &str) -> Vec<Tok> {
    let chars: Vec<char> = pat.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' => {
                i += 1;
                let mut lit = String::new();
                while i < chars.len() && chars[i] != '\'' {
                    lit.push(chars[i]);
                    i += 1;
                }
                i += 1;
                out.push(Tok::Lit(lit));
            }
            '\\' if i + 1 < chars.len() => {
                out.push(Tok::Lit(chars[i + 1].to_string()));
                i += 2;
            }
            'd' | 'm' | 'y' | 'h' | 'H' | 'M' | 's' | 't' => {
                let mut n = 1;
                while i + n < chars.len() && chars[i + n] == c {
                    n += 1;
                }
                out.push(Tok::Field(c, n));
                i += n;
            }
            _ => {
                out.push(Tok::Lit(c.to_string()));
                i += 1;
            }
        }
    }
    out
}

/// Formatea una fecha con un patrón de Acrobat (`dd/mm/yyyy`, `mmmm d, yyyy`, `HH:MM`, `h:MM:ss tt`...).
/// En el patrón `m` es el mes y `M` los minutos.
pub fn format_date(pattern: &str, p: &DateParts, lang: Language) -> String {
    let mut s = String::new();
    for t in tokenize_pattern(pattern) {
        match t {
            Tok::Lit(l) => s.push_str(&l),
            Tok::Field(c, n) => {
                let h12 = if p.hour.is_multiple_of(12) { 12 } else { p.hour % 12 };
                match (c, n) {
                    ('d', 1) => write!(s, "{}", p.day).ok(),
                    ('d', 2) => write!(s, "{:02}", p.day).ok(),
                    ('d', 3) => {
                        s.push_str(lang.days_short()[weekday(p.year, p.month, p.day) as usize]);
                        Some(())
                    }
                    ('d', _) => {
                        s.push_str(lang.days()[weekday(p.year, p.month, p.day) as usize]);
                        Some(())
                    }
                    ('m', 1) => write!(s, "{}", p.month + 1).ok(),
                    ('m', 2) => write!(s, "{:02}", p.month + 1).ok(),
                    ('m', 3) => {
                        s.push_str(lang.months_short()[p.month as usize % 12]);
                        Some(())
                    }
                    ('m', _) => {
                        s.push_str(lang.months()[p.month as usize % 12]);
                        Some(())
                    }
                    ('y', 1 | 2) => write!(s, "{:02}", p.year.rem_euclid(100)).ok(),
                    ('y', 3) => write!(s, "{:03}", p.year.rem_euclid(1000)).ok(),
                    ('y', _) => write!(s, "{:04}", p.year).ok(),
                    ('h', 1) => write!(s, "{h12}").ok(),
                    ('h', _) => write!(s, "{h12:02}").ok(),
                    ('H', 1) => write!(s, "{}", p.hour).ok(),
                    ('H', _) => write!(s, "{:02}", p.hour).ok(),
                    ('M', 1) => write!(s, "{}", p.minute).ok(),
                    ('M', _) => write!(s, "{:02}", p.minute).ok(),
                    ('s', 1) => write!(s, "{}", p.second).ok(),
                    ('s', _) => write!(s, "{:02}", p.second).ok(),
                    ('t', 1) => write!(s, "{}", if p.hour < 12 { "a" } else { "p" }).ok(),
                    ('t', _) => write!(s, "{}", if p.hour < 12 { "am" } else { "pm" }).ok(),
                    _ => None,
                };
            }
        }
    }
    s
}

#[derive(Debug, PartialEq, Eq)]
enum InTok {
    Num(String),
    Alpha(String),
}

fn tokenize_input(s: &str) -> Vec<InTok> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut numeric = true;
    let flush = |cur: &mut String, numeric: bool, out: &mut Vec<InTok>| {
        if !cur.is_empty() {
            let t = std::mem::take(cur);
            out.push(if numeric { InTok::Num(t) } else { InTok::Alpha(t) });
        }
    };
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !numeric {
                flush(&mut cur, numeric, &mut out);
            }
            numeric = true;
            cur.push(c);
        } else if c.is_alphabetic() {
            if numeric {
                flush(&mut cur, numeric, &mut out);
            }
            numeric = false;
            cur.push(c);
        } else {
            flush(&mut cur, numeric, &mut out);
        }
    }
    flush(&mut cur, numeric, &mut out);
    out
}

fn month_from_name(name: &str, lang: Language) -> Option<u32> {
    let n = name.to_lowercase();
    if n.chars().count() < 3 {
        return None;
    }
    // Se aceptan los dos idiomas: un formulario en español puede recibir «Nov» y uno en inglés «noviembre».
    for l in [lang, Language::Es, Language::En] {
        for (i, (long, short)) in l.months().iter().zip(l.months_short()).enumerate() {
            let (long, short) = (long.to_lowercase(), short.to_lowercase());
            if long == n || short == n || (n.chars().count() >= 3 && long.starts_with(&n)) {
                return Some(i as u32);
            }
        }
    }
    None
}

fn two_digit_year(y: i32) -> i32 {
    if y < 50 { 2000 + y } else { 1900 + y }
}

/// Interpreta una fecha según un patrón. Acepta cualquier separador, nombres de mes y, si el texto es todo cifras,
/// la forma compacta (`03112026` para `dd/mm/yyyy`). Los datos que faltan se toman de hoy (año y mes) o de las
/// 00:00. `None` si no encaja o la fecha no existe (31 de febrero...).
pub fn parse_date(input: &str, pattern: &str, lang: Language, now_ms: Option<f64>) -> Option<DateParts> {
    let toks = tokenize_pattern(pattern);
    let fields: Vec<(char, usize)> = toks
        .iter()
        .filter_map(|t| match t {
            Tok::Field(c, n) => Some((*c, *n)),
            Tok::Lit(_) => None,
        })
        .collect();
    let input_toks = tokenize_input(input.trim());
    let (ty, tm, _) = today(now_ms);

    let mut y: Option<i32> = None;
    let mut mo: Option<u32> = None;
    let mut d: Option<u32> = None;
    let (mut h, mut mi, mut se) = (None::<u32>, 0u32, 0u32);
    let mut pm: Option<bool> = None;

    // Forma compacta: una sola tira de cifras.
    let compact: Option<Vec<String>> = match input_toks.as_slice() {
        [InTok::Num(n)]
            if fields.len() > 1
                && fields
                    .iter()
                    .all(|(c, k)| !matches!((c, k), ('m', 3..) | ('d', 3..) | ('t', _))) =>
        {
            let widths: Vec<usize> = fields
                .iter()
                .map(|(c, k)| match (c, k) {
                    ('y', 3..) => 4,
                    _ => 2,
                })
                .collect();
            (widths.iter().sum::<usize>() == n.len()).then(|| {
                let mut at = 0;
                widths
                    .iter()
                    .map(|w| {
                        let part = n[at..at + w].to_string();
                        at += w;
                        part
                    })
                    .collect()
            })
        }
        _ => None,
    };

    let mut it = input_toks.iter().peekable();
    let mut compact_it = compact.as_ref().map(|c| c.iter());
    for (c, n) in &fields {
        let wants_alpha = matches!((c, n), ('m', 3..) | ('d', 3..) | ('t', _));
        if wants_alpha {
            // Un nombre de mes o día; si el texto trae un número en su lugar, el mes numérico también vale.
            match it.peek() {
                Some(InTok::Alpha(a)) => {
                    let a = a.clone();
                    it.next();
                    match c {
                        'm' => mo = Some(month_from_name(&a, lang)?),
                        't' => {
                            let l = a.to_lowercase();
                            pm = Some(match l.as_str() {
                                "pm" | "p" => true,
                                "am" | "a" => false,
                                _ => return None,
                            });
                        }
                        _ => {}
                    }
                }
                Some(InTok::Num(num)) if *c == 'm' => {
                    let v: u32 = num.parse().ok()?;
                    it.next();
                    mo = Some(v.checked_sub(1)?);
                }
                _ if *c == 't' => {} // la marca am/pm es opcional
                _ if *c == 'd' => {}
                _ => return None,
            }
            continue;
        }
        let tok: String = if let Some(ci) = compact_it.as_mut() {
            ci.next()?.clone()
        } else {
            match it.next()? {
                InTok::Num(num) => num.clone(),
                InTok::Alpha(a) if *c == 'm' => {
                    // «3 nov 2026» con patrón numérico d/m/y.
                    mo = Some(month_from_name(a, lang)?);
                    continue;
                }
                InTok::Alpha(_) => return None,
            }
        };
        let v: i64 = tok.parse().ok()?;
        match c {
            'd' => d = Some(u32::try_from(v).ok()?),
            'm' => mo = Some(u32::try_from(v).ok()?.checked_sub(1)?),
            'y' => {
                y = Some(if tok.len() <= 2 {
                    two_digit_year(v as i32)
                } else {
                    v as i32
                })
            }
            'h' | 'H' => h = Some(u32::try_from(v).ok()?),
            'M' => mi = u32::try_from(v).ok()?,
            's' => se = u32::try_from(v).ok()?,
            _ => {}
        }
    }
    let mut hour = h.unwrap_or(0);
    if let Some(is_pm) = pm {
        if hour > 12 {
            return None;
        }
        hour = match (is_pm, hour) {
            (true, 12) => 12,
            (true, x) => x + 12,
            (false, 12) => 0,
            (false, x) => x,
        };
    }
    let p = DateParts {
        year: y.unwrap_or(ty),
        month: mo.unwrap_or(if d.is_some() && y.is_none() { tm } else { 0 }),
        day: d.unwrap_or(1),
        hour,
        minute: mi,
        second: se,
    };
    p.is_valid().then_some(p)
}

// --- Números -----------------------------------------------------------------------------------------------------

/// Convierte un número en el texto que se guarda en un campo: hasta 15 cifras significativas, sin ceros de más
/// (`3.3000000000000003` pasa a `3.3`; `1e21` a `1e+21`).
pub fn number_to_field_text(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mut s = format!("{v:.15e}");
    // `s` = «d.dddddddddddddddde±x»; redondea a 15 cifras significativas.
    if let Ok(rounded) = format!("{v:.14e}").parse::<f64>() {
        s = format!("{rounded}");
        if s.contains('e') || s.len() > 24 {
            s = format!("{rounded:e}");
        }
        return s;
    }
    s
}

/// Estilo de separadores de `AFNumber_Format` / `%,N` de `printf`: 0 = `1,234.56`, 1 = `1234.56`, 2 = `1.234,56`,
/// 3 = `1234,56`, 4 = `1'234.56`.
pub fn separators(style: i64) -> (Option<char>, char) {
    match style {
        0 => (Some(','), '.'),
        1 => (None, '.'),
        2 => (Some('.'), ','),
        3 => (None, ','),
        _ => (Some('\''), '.'),
    }
}

/// `abs` con `decimals` decimales, redondeando los empates exactos hacia arriba (como `toFixed` de JavaScript;
/// Rust redondea al par: `0.125` daría `0.12`).
fn round_half_up(abs: f64, decimals: usize) -> String {
    let normal = format!("{abs:.decimals$}");
    let long = format!("{abs:.60}");
    let Some(dot) = long.find('.') else { return normal };
    let keep = dot + 1 + decimals;
    let tail = &long[keep.min(long.len())..];
    if !(tail.starts_with('5') && tail[1..].bytes().all(|b| b == b'0')) {
        return normal;
    }
    // Empate exacto: suma una unidad en el último decimal conservado.
    let mut digits: Vec<u8> = long[..keep].bytes().collect();
    let mut i = digits.len();
    loop {
        if i == 0 {
            digits.insert(0, b'1');
            break;
        }
        i -= 1;
        match digits[i] {
            b'.' => continue,
            b'9' => digits[i] = b'0',
            d => {
                digits[i] = d + 1;
                break;
            }
        }
    }
    let mut s = String::from_utf8(digits).unwrap_or(normal);
    if decimals == 0 && s.ends_with('.') {
        s.pop();
    }
    s
}

/// Número con `decimals` decimales y el estilo de separadores dado (siempre con signo `-` si es negativo).
pub fn format_number(v: f64, decimals: usize, sep_style: i64) -> String {
    let (thousands, decimal) = separators(sep_style);
    let body = round_half_up(v.abs(), decimals);
    let neg = v < 0.0 && body.chars().any(|c| c != '0' && c != '.');
    let (int, frac) = body.split_once('.').map_or((body.as_str(), ""), |(a, b)| (a, b));
    let mut int_out = String::new();
    if let Some(t) = thousands {
        let n = int.len();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (n - i) % 3 == 0 {
                int_out.push(t);
            }
            int_out.push(c);
        }
    } else {
        int_out.push_str(int);
    }
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    s.push_str(&int_out);
    if decimals > 0 {
        s.push(decimal);
        s.push_str(frac);
    }
    s
}

/// `AFMakeNumber`: extrae el número de un texto (admite coma o punto como decimal, miles, paréntesis y signo).
/// `None` si no hay ninguna cifra.
pub fn make_number(s: &str) -> Option<f64> {
    let mut neg = false;
    let mut buf = String::new();
    let t = s.trim();
    if t.starts_with('(') && t.ends_with(')') {
        neg = true;
    }
    for c in t.chars() {
        match c {
            '0'..='9' | '.' | ',' => buf.push(c),
            '-' | '\u{2212}' if buf.is_empty() => neg = true,
            '+' if buf.is_empty() => {}
            ' ' | '\u{a0}' | '\'' => {}
            _ => {
                if buf.chars().any(|c| c.is_ascii_digit()) {
                    // Un símbolo tras las cifras (%, €, texto): termina el número.
                    if c == '-' {
                        neg = true;
                    }
                    break;
                }
            }
        }
    }
    if !buf.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let dots = buf.matches('.').count();
    let commas = buf.matches(',').count();
    let decimal_sep = if dots > 0 && commas > 0 {
        // Ambos: el último es el decimal.
        if buf.rfind('.') > buf.rfind(',') {
            Some('.')
        } else {
            Some(',')
        }
    } else if dots > 1 || commas > 1 {
        None
    } else if dots == 1 {
        Some('.')
    } else if commas == 1 {
        Some(',')
    } else {
        None
    };
    let mut clean = String::new();
    for c in buf.chars() {
        if c.is_ascii_digit() {
            clean.push(c);
        } else if Some(c) == decimal_sep && !clean.contains('.') {
            clean.push('.');
        }
    }
    let v: f64 = clean.parse().ok()?;
    Some(if neg { -v } else { v })
}

/// ¿Es todo el texto algo que `make_number` puede leer sin descartar letras? (validación de teclado de
/// `AFNumber_Keystroke`: cifras, signo, separadores, espacios, paréntesis, `%` y el símbolo de moneda.)
pub fn is_numeric_text(s: &str, currency: &str) -> bool {
    let rest = if currency.is_empty() {
        s.to_string()
    } else {
        s.replace(currency, "")
    };
    let rest = rest.trim();
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | '(' | ')' | '%' | ' ' | '\u{a0}' | '\''))
        && make_number(rest).is_some()
}

// --- printf ------------------------------------------------------------------------------------------------------

/// Argumento de `util.printf`.
#[derive(Debug, Clone, PartialEq)]
pub enum PrintfArg {
    Num(f64),
    Str(String),
}

impl PrintfArg {
    fn num(&self) -> f64 {
        match self {
            PrintfArg::Num(n) => *n,
            PrintfArg::Str(s) => make_number(s).unwrap_or(0.0),
        }
    }
    fn text(&self) -> String {
        match self {
            PrintfArg::Num(n) => number_to_field_text(*n),
            PrintfArg::Str(s) => s.clone(),
        }
    }
}

/// `util.printf(fmt, ...)`: `%d`, `%f`, `%s`, `%x` con banderas (`+`, espacio, `0`, `-`, `#`), ancho, precisión y la
/// extensión de Acrobat `%,N.Mf` (estilo de separadores N, M decimales). `%%` es un porcentaje.
pub fn printf(fmt: &str, args: &[PrintfArg]) -> String {
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut next = 0;
    while i < chars.len() {
        if chars[i] != '%' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= chars.len() {
            out.push('%');
            break;
        }
        if chars[i] == '%' {
            out.push('%');
            i += 1;
            continue;
        }
        let (mut plus, mut space, mut zero, mut left, mut alt) = (false, false, false, false, false);
        while i < chars.len() {
            match chars[i] {
                '+' => plus = true,
                ' ' => space = true,
                '0' => zero = true,
                '-' => left = true,
                '#' => alt = true,
                _ => break,
            }
            i += 1;
        }
        let mut width = 0usize;
        while i < chars.len() && chars[i].is_ascii_digit() {
            width = width * 10 + chars[i].to_digit(10).unwrap_or(0) as usize;
            i += 1;
        }
        let mut sep = None;
        if i + 1 < chars.len() && chars[i] == ',' && chars[i + 1].is_ascii_digit() {
            sep = chars[i + 1].to_digit(10).map(i64::from);
            i += 2;
        }
        let mut prec = None;
        if i < chars.len() && chars[i] == '.' {
            i += 1;
            let mut p = 0usize;
            while i < chars.len() && chars[i].is_ascii_digit() {
                p = p * 10 + chars[i].to_digit(10).unwrap_or(0) as usize;
                i += 1;
            }
            prec = Some(p);
        }
        let Some(&conv) = chars.get(i) else { break };
        i += 1;
        let arg = args.get(next);
        next += 1;
        let Some(arg) = arg else { continue };
        let mut body = match conv {
            'd' | 'i' | 'u' => {
                let v = arg.num().trunc();
                let mut s = format_number(v, 0, sep.unwrap_or(1));
                if let Some(p) = prec {
                    let neg = s.starts_with('-');
                    let digits = s.trim_start_matches('-').to_string();
                    s = format!(
                        "{}{}{digits}",
                        if neg { "-" } else { "" },
                        "0".repeat(p.saturating_sub(digits.len()))
                    );
                }
                s
            }
            'f' | 'F' => format_number(arg.num(), prec.unwrap_or(6), sep.unwrap_or(1)),
            'e' | 'E' => {
                let s = format!("{:.*e}", prec.unwrap_or(6), arg.num());
                if conv == 'E' { s.to_uppercase() } else { s }
            }
            'g' | 'G' => number_to_field_text(arg.num()),
            'x' | 'X' => {
                let v = arg.num().trunc() as i64;
                let s = if conv == 'x' {
                    format!("{v:x}")
                } else {
                    format!("{v:X}")
                };
                if alt { format!("0{conv}{s}") } else { s }
            }
            'o' => format!("{:o}", arg.num().trunc() as i64),
            'c' => char::from_u32(arg.num() as u32).map(String::from).unwrap_or_default(),
            's' => {
                let t = arg.text();
                match prec {
                    Some(p) => t.chars().take(p).collect(),
                    None => t,
                }
            }
            other => {
                out.push('%');
                out.push(other);
                continue;
            }
        };
        if matches!(conv, 'd' | 'i' | 'f' | 'F' | 'e' | 'E' | 'g' | 'G') && !body.starts_with('-') {
            if plus {
                body.insert(0, '+');
            } else if space {
                body.insert(0, ' ');
            }
        }
        let len = body.chars().count();
        if len < width {
            let pad = width - len;
            if left {
                body.push_str(&" ".repeat(pad));
            } else if zero && conv != 's' {
                let at = usize::from(body.starts_with(['-', '+', ' ']));
                body.insert_str(at, &"0".repeat(pad));
            } else {
                body.insert_str(0, &" ".repeat(pad));
            }
        }
        out.push_str(&body);
    }
    out
}

// --- Máscaras de teclado -----------------------------------------------------------------------------------------

/// ¿Encaja `c` en la posición de máscara `m`? `9` cifra, `A` letra, `O` letra o cifra, `X` cualquier carácter; el
/// resto de la máscara son literales que el texto debe repetir.
pub fn mask_accepts(m: char, c: char) -> bool {
    match m {
        '9' => c.is_ascii_digit(),
        'A' => c.is_alphabetic(),
        'O' => c.is_alphanumeric(),
        'X' => true,
        lit => c == lit,
    }
}

/// ¿Es `text` una máscara completa (o, con `partial`, el principio de una)?
pub fn mask_matches(mask: &str, text: &str, partial: bool) -> bool {
    let m: Vec<char> = mask.chars().collect();
    let t: Vec<char> = text.chars().collect();
    if t.len() > m.len() || (!partial && t.len() != m.len()) {
        return false;
    }
    t.iter().zip(&m).all(|(c, m)| mask_accepts(*m, *c))
}

/// Aplica una forma de `AFSpecial_Format`: `0` código postal, `1` código postal + 4, `2` teléfono, `3` nº de la
/// Seguridad Social de EE. UU. Solo cuenta las cifras del texto.
pub fn special_format(kind: i64, text: &str) -> String {
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    let apply = |mask: &str| {
        let mut it = digits.chars();
        let mut s = String::new();
        for m in mask.chars() {
            if m == '9' {
                match it.next() {
                    Some(d) => s.push(d),
                    None => break,
                }
            } else {
                s.push(m);
            }
        }
        s
    };
    match kind {
        0 => digits,
        1 => apply("99999-9999"),
        2 => match digits.len() {
            7 => apply("999-9999"),
            10 => apply("(999) 999-9999"),
            _ => text.to_string(),
        },
        3 => apply("999-99-9999"),
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> DateParts {
        DateParts {
            year: y,
            month: m,
            day,
            hour: 0,
            minute: 0,
            second: 0,
        }
    }

    #[test]
    fn weekday_and_today() {
        assert_eq!(weekday(2026, 10, 5), 4, "5 nov 2026 es jueves");
        assert_eq!(weekday(1970, 0, 1), 4);
        assert_eq!(today(Some(0.0)), (1970, 0, 1));
        assert_eq!(today(Some(1_794_000_000_000.0)).0, 2026);
    }

    #[test]
    fn formats_dates() {
        let p = DateParts {
            hour: 15,
            minute: 7,
            second: 9,
            ..d(2026, 10, 5)
        };
        assert_eq!(format_date("dd/mm/yyyy", &p, Language::Es), "05/11/2026");
        assert_eq!(format_date("d-mmm-yy", &p, Language::En), "5-Nov-26");
        assert_eq!(
            format_date("dddd, d 'de' mmmm", &p, Language::Es),
            "jueves, 5 de noviembre"
        );
        assert_eq!(format_date("h:MM:ss tt", &p, Language::Es), "3:07:09 pm");
        assert_eq!(format_date("HH:MM", &p, Language::Es), "15:07");
        assert_eq!(format_date("yyyy-mm-dd\\T", &p, Language::Es), "2026-11-05T");
    }

    #[test]
    fn parses_dates() {
        let es = Language::Es;
        assert_eq!(parse_date("03/11/2026", "dd/mm/yyyy", es, None), Some(d(2026, 10, 3)));
        assert_eq!(parse_date("3-11-26", "dd/mm/yyyy", es, None), Some(d(2026, 10, 3)));
        assert_eq!(parse_date("03112026", "dd/mm/yyyy", es, None), Some(d(2026, 10, 3)));
        assert_eq!(parse_date("3 nov 2026", "d/m/yyyy", es, None), Some(d(2026, 10, 3)));
        assert_eq!(
            parse_date("5 de noviembre 2026", "d mmmm yyyy", es, None),
            None,
            "«de» no es un mes"
        );
        assert_eq!(
            parse_date("noviembre 5, 2026", "mmmm d, yyyy", es, None),
            Some(d(2026, 10, 5))
        );
        assert_eq!(parse_date("31/02/2026", "dd/mm/yyyy", es, None), None);
        assert_eq!(parse_date("29/02/2028", "dd/mm/yyyy", es, None), Some(d(2028, 1, 29)));
        assert_eq!(parse_date("hola", "dd/mm/yyyy", es, None), None);
        let t = parse_date("3:07 pm", "h:MM tt", es, None).unwrap();
        assert_eq!((t.hour, t.minute), (15, 7));
        assert_eq!(parse_date("12:00 am", "h:MM tt", es, None).map(|t| t.hour), Some(0));
        assert_eq!(parse_date("25:00", "HH:MM", es, None), None);
        assert_eq!(
            parse_date("14:30", "HH:MM", es, None).map(|t| (t.hour, t.minute)),
            Some((14, 30))
        );
        // Sin año: el de hoy.
        assert_eq!(
            parse_date("3/11", "d/m", es, Some(1_794_000_000_000.0)).map(|p| p.year),
            Some(2026)
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(format_number(1234.5, 2, 2), "1.234,50");
        assert_eq!(format_number(1234.5, 2, 0), "1,234.50");
        assert_eq!(format_number(-1234567.891, 1, 4), "-1'234'567.9");
        assert_eq!(format_number(0.0, 0, 3), "0");
        assert_eq!(format_number(-0.001, 2, 1), "0.00", "sin «-0,00»");
        assert_eq!(number_to_field_text(3.3000000000000003), "3.3");
        assert_eq!(number_to_field_text(6.6), "6.6");
        assert_eq!(number_to_field_text(-12.0), "-12");
        assert_eq!(number_to_field_text(0.1 + 0.2), "0.3");
        assert_eq!(make_number("1.234,56"), Some(1234.56));
        assert_eq!(make_number("1,234.56"), Some(1234.56));
        assert_eq!(make_number("1,5"), Some(1.5));
        assert_eq!(make_number("1.234.567"), Some(1_234_567.0));
        assert_eq!(make_number("-3"), Some(-3.0));
        assert_eq!(make_number("(5)"), Some(-5.0));
        assert_eq!(make_number("12,50 €"), Some(12.5));
        assert_eq!(make_number("abc"), None);
        assert_eq!(make_number(""), None);
        assert!(is_numeric_text("1.234,50 €", "€"));
        assert!(!is_numeric_text("12abc", "€"));
        assert!(!is_numeric_text("", ""));
    }

    #[test]
    fn printf_formats() {
        use PrintfArg::*;
        assert_eq!(printf("%,2.2f", &[Num(1234.5)]), "1.234,50");
        assert_eq!(printf("%d items", &[Num(3.9)]), "3 items");
        assert_eq!(
            printf("[%5d][%-5d][%05d]", &[Num(42.0), Num(42.0), Num(42.0)]),
            "[   42][42   ][00042]"
        );
        assert_eq!(
            printf("%+.1f%%", &[Num(2.25)]),
            "+2.3%",
            "empate: hacia arriba, como toFixed de JavaScript"
        );
        assert_eq!(printf("%.2f", &[Num(0.125)]), "0.13");
        assert_eq!(printf("%.2f", &[Num(1.005)]), "1.00", "1,005 en binario es 1,00499...");
        assert_eq!(printf("%s-%s", &[Str("a".into()), Str("b".into())]), "a-b");
        assert_eq!(printf("%x %X", &[Num(255.0), Num(255.0)]), "ff FF");
        assert_eq!(printf("%.3s", &[Str("abcdef".into())]), "abc");
        assert_eq!(printf("sin args %d", &[]), "sin args ");
        assert_eq!(printf("%08.3f", &[Num(-7.12345)]), "-007.123");
    }

    #[test]
    fn masks_and_special() {
        assert!(mask_matches("999-AA", "123-AB", false));
        assert!(!mask_matches("999-AA", "123-A1", false));
        assert!(mask_matches("999-AA", "12", true));
        assert!(!mask_matches("999-AA", "12", false));
        assert!(!mask_matches("999-AA", "1a", true));
        assert_eq!(special_format(1, "123456789"), "12345-6789");
        assert_eq!(special_format(2, "6125551234"), "(612) 555-1234");
        assert_eq!(special_format(2, "5551234"), "555-1234");
        assert_eq!(special_format(3, "123456789"), "123-45-6789");
        assert_eq!(special_format(0, "28001"), "28001");
    }
}
