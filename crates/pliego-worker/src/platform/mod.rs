//! Restricciones del proceso worker según el sistema operativo.

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::Sandbox;

/// En otros sistemas no hay restricciones adicionales (el aislamiento es solo por proceso).
#[cfg(not(windows))]
pub struct Sandbox;

#[cfg(not(windows))]
impl Sandbox {
    pub fn attach(_child: &std::process::Child, _memory_limit: Option<usize>) -> std::io::Result<Option<Self>> {
        Ok(None)
    }
}
