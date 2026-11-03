//! Registro de Windows: envoltorio mínimo sobre la API.
#![allow(unsafe_code)]

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::*;
use windows::core::{PCWSTR, Result};

#[derive(Clone, Copy)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

impl Hive {
    fn raw(self) -> HKEY {
        match self {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        }
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn ok(e: WIN32_ERROR) -> Result<()> {
    e.ok()
}

pub struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: `self.0` es una clave abierta por esta estructura y no se usa después.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

impl Key {
    /// Abre o crea (con todos los niveles intermedios) para escribir.
    pub fn create(hive: Hive, sub: &str) -> Result<Key> {
        let sub = wide(sub);
        let mut out = HKEY::default();
        // SAFETY: `sub` es UTF-16 terminado en cero y `out` es un destino válido durante la llamada.
        unsafe {
            ok(RegCreateKeyExW(
                hive.raw(),
                PCWSTR(sub.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_READ | KEY_WRITE,
                None,
                &mut out,
                None,
            ))?;
        }
        Ok(Key(out))
    }

    fn open_with(hive: Hive, sub: &str, access: REG_SAM_FLAGS) -> Option<Key> {
        let sub = wide(sub);
        let mut out = HKEY::default();
        // SAFETY: igual que en `create`.
        let r = unsafe { RegOpenKeyExW(hive.raw(), PCWSTR(sub.as_ptr()), None, access, &mut out) };
        (r == ERROR_SUCCESS).then_some(Key(out))
    }

    pub fn open(hive: Hive, sub: &str) -> Option<Key> {
        Key::open_with(hive, sub, KEY_READ)
    }

    pub fn open_write(hive: Hive, sub: &str) -> Option<Key> {
        Key::open_with(hive, sub, KEY_READ | KEY_WRITE)
    }

    fn set_raw(&self, name: &str, kind: REG_VALUE_TYPE, data: &[u8]) -> Result<()> {
        let name = wide(name);
        // SAFETY: `name` termina en cero y `data` es un trozo de memoria válido durante la llamada.
        unsafe { ok(RegSetValueExW(self.0, PCWSTR(name.as_ptr()), None, kind, Some(data))) }
    }

    fn utf16_bytes(s: &str) -> Vec<u8> {
        wide(s).iter().flat_map(|u| u.to_le_bytes()).collect()
    }

    pub fn set_string(&self, name: &str, value: &str) -> Result<()> {
        self.set_raw(name, REG_SZ, &Key::utf16_bytes(value))
    }

    pub fn set_expand_string(&self, name: &str, value: &str) -> Result<()> {
        self.set_raw(name, REG_EXPAND_SZ, &Key::utf16_bytes(value))
    }

    pub fn set_dword(&self, name: &str, value: u32) -> Result<()> {
        self.set_raw(name, REG_DWORD, &value.to_le_bytes())
    }

    /// Valor sin datos (`REG_NONE`), como los de `OpenWithProgids`.
    pub fn set_empty(&self, name: &str) -> Result<()> {
        self.set_raw(name, REG_NONE, &[])
    }

    fn get_raw(&self, name: &str) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
        let name = wide(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut len = 0u32;
        // SAFETY: primera llamada solo para conocer el tamaño; los punteros son válidos.
        let r = unsafe {
            RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut len),
            )
        };
        if r != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        // SAFETY: `buf` tiene `len` bytes; la API escribe como mucho `len`.
        let r = unsafe {
            RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                Some(buf.as_mut_ptr()),
                Some(&mut len),
            )
        };
        (r == ERROR_SUCCESS).then(|| {
            buf.truncate(len as usize);
            (kind, buf)
        })
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
        let (kind, buf) = self.get_raw(name)?;
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return None;
        }
        let units: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Some(String::from_utf16_lossy(&units).trim_end_matches('\0').to_string())
    }

    pub fn get_dword(&self, name: &str) -> Option<u32> {
        let (kind, buf) = self.get_raw(name)?;
        (kind == REG_DWORD && buf.len() == 4).then(|| u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))
    }

    pub fn delete_value(&self, name: &str) -> Result<()> {
        let name = wide(name);
        // SAFETY: `name` termina en cero.
        let r = unsafe { RegDeleteValueW(self.0, PCWSTR(name.as_ptr())) };
        if r == ERROR_FILE_NOT_FOUND { Ok(()) } else { ok(r) }
    }

    /// Borra una clave y todo lo que cuelga de ella; que no exista no es un error.
    pub fn delete_tree(hive: Hive, sub: &str) -> Result<()> {
        let sub = wide(sub);
        // SAFETY: `sub` termina en cero.
        let r = unsafe { RegDeleteTreeW(hive.raw(), PCWSTR(sub.as_ptr())) };
        if r == ERROR_FILE_NOT_FOUND { Ok(()) } else { ok(r) }
    }

    /// Borra la clave solo si no tiene subclaves ni valores.
    pub fn delete_if_empty(hive: Hive, sub: &str) {
        let wsub = wide(sub);
        // SAFETY: `RegDeleteKeyW` falla sin borrar si la clave tiene subclaves.
        unsafe {
            let _ = RegDeleteKeyW(hive.raw(), PCWSTR(wsub.as_ptr()));
        }
    }
}
