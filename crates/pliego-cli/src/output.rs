//! `println!`/`eprintln!` de la CLI: igual que los de `std`, pero traducen el texto cuando la sesión está en inglés
//! (ver `i18n.rs`). Se declaran antes que el resto de módulos, así los sustituyen sin tocar cada llamada.

macro_rules! println {
    () => { ::std::println!() };
    ($($t:tt)*) => { ::std::println!("{}", $crate::i18n::tr_lines(&::std::format!($($t)*))) };
}

macro_rules! eprintln {
    () => { ::std::eprintln!() };
    ($($t:tt)*) => { ::std::eprintln!("{}", $crate::i18n::tr_lines(&::std::format!($($t)*))) };
}

macro_rules! eprint {
    ($($t:tt)*) => { ::std::eprint!("{}", $crate::i18n::tr_lines(&::std::format!($($t)*))) };
}
