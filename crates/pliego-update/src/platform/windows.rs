//! Windows: directiva `HKLM\SOFTWARE\Policies\Pliego\Updates`, compilación del sistema, conexión de uso medido
//! (WinRT) y carpetas de programas.
#![allow(unsafe_code)]

use std::path::PathBuf;

use windows::Networking::Connectivity::{NetworkCostType, NetworkInformation};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW};
use windows::core::PCWSTR;

use crate::config::Policy;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn get_dword(hive: HKEY, key: &str, name: &str) -> Option<u32> {
    let (key, name) = (wide(key), wide(name));
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `key` y `name` terminan en cero; `value` y `size` son locales válidos durante la llamada y `size`
    // indica el tamaño real del búfer.
    let r = unsafe {
        RegGetValueW(
            hive,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(std::ptr::from_mut(&mut value).cast()),
            Some(&mut size),
        )
    };
    (r == ERROR_SUCCESS).then_some(value)
}

fn get_string(hive: HKEY, key: &str, name: &str) -> Option<String> {
    let (key, name) = (wide(key), wide(name));
    let mut size = 0u32;
    // SAFETY: primera llamada sin búfer, solo para conocer el tamaño en bytes.
    let r = unsafe {
        RegGetValueW(
            hive,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    if r != ERROR_SUCCESS || size < 2 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: `buf` tiene al menos `size` bytes; la API escribe como mucho `size` y termina el texto en cero.
    let r = unsafe {
        RegGetValueW(
            hive,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if r != ERROR_SUCCESS {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// Directiva de la organización (vacía si no hay ninguna).
pub fn policy() -> Policy {
    const KEY: &str = r"SOFTWARE\Policies\Pliego\Updates";
    let text = |n: &str| get_string(HKEY_LOCAL_MACHINE, KEY, n);
    Policy::from_values(
        get_dword(HKEY_LOCAL_MACHINE, KEY, "Disabled"),
        text("Url").as_deref(),
        text("Channel").as_deref(),
        text("Mode").as_deref(),
    )
}

/// Compilación de Windows (p. ej. 19045), para `min_windows_build`.
pub fn os_build() -> Option<u32> {
    get_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "CurrentBuildNumber",
    )?
    .trim()
    .parse()
    .ok()
}

/// ¿La conexión a Internet actual es de uso medido (o con límite de datos)?
pub fn is_metered() -> bool {
    let cost = NetworkInformation::GetInternetConnectionProfile().and_then(|p| p.GetConnectionCost());
    cost.and_then(|c| c.NetworkCostType())
        .is_ok_and(|t| t == NetworkCostType::Fixed || t == NetworkCostType::Variable)
}

/// `Program Files` (y su variante x86), para distinguir la instalación por equipo.
pub fn program_dirs() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect()
}
