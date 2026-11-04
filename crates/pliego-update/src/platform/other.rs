//! Fuera de Windows: sin directiva, sin conexión medida y sin compilación de Windows.

use std::path::PathBuf;

use crate::config::Policy;

pub fn policy() -> Policy {
    Policy::default()
}

pub fn is_metered() -> bool {
    false
}

pub fn os_build() -> Option<u32> {
    None
}

pub fn program_dirs() -> Vec<PathBuf> {
    Vec::new()
}
