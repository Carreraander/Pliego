//! Lo que depende del sistema operativo: directiva de la organización, conexión de uso medido y carpetas.
//! En Windows se leen del registro y de WinRT (`windows.rs`); fuera de Windows no hay nada de eso (`other.rs`).

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{is_metered, os_build, policy, program_dirs};

#[cfg(not(windows))]
mod other;
#[cfg(not(windows))]
pub use other::{is_metered, os_build, policy, program_dirs};
