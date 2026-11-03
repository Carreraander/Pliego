//! Windows: `AppUserModelID`, cuadro de error y procesos sin ventana. Declaraciones mínimas para no depender
//! del crate `windows` y mantener el lanzador pequeño.
#![allow(unsafe_code)]

use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;
const MB_OK_ICONERROR: u32 = 0x0000_0010;

#[link(name = "shell32")]
unsafe extern "system" {
    fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(hwnd: *mut core::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Agrupa el lanzador con la app en la barra de tareas.
pub fn init() {
    let id = wide("Pliego.App");
    // SAFETY: `id` es una cadena UTF-16 terminada en cero que vive durante toda la llamada.
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
    }
}

pub fn error_box(msg: &str) {
    let (text, caption) = (wide(msg), wide("Pliego"));
    // SAFETY: ambos búferes son UTF-16 terminados en cero y válidos durante la llamada; sin ventana propietaria.
    unsafe {
        MessageBoxW(core::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK_ICONERROR);
    }
}

pub fn detach(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
}
