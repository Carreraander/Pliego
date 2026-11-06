//! Procesos, accesos directos, carpetas conocidas y avisos al Explorador.
#![allow(unsafe_code)]

use super::reg::wide;
use crate::model::Scope;
use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::*;
use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::Com::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::PropertiesSystem::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, Interface, PCWSTR, PWSTR};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn locale_name() -> Option<String> {
    let mut buf = [0u16; 85];
    // SAFETY: `buf` es un búfer propio de 85 elementos (LOCALE_NAME_MAX_LENGTH).
    let n = unsafe { GetUserDefaultLocaleName(&mut buf) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
}

pub fn is_elevated() -> bool {
    let mut token = HANDLE::default();
    // SAFETY: se abre el token del propio proceso, se consulta con un búfer del tamaño exacto y se cierra.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elev = TOKEN_ELEVATION::default();
        let mut ret = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elev as *mut _ as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elev.TokenIsElevated != 0
    }
}

pub fn free_space(path: &Path) -> Option<u64> {
    let p = wide(&path.display().to_string());
    let mut avail = 0u64;
    // SAFETY: `p` termina en cero y `avail` es un destino válido.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(p.as_ptr()), Some(&mut avail), None, None).ok()? };
    Some(avail)
}

fn image_path(pid: u32) -> Option<String> {
    // SAFETY: se abre el proceso solo para consultar su ruta, con un búfer propio, y se cierra al terminar.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// PIDs de `pliego-app.exe` que se ejecutan desde `root`.
pub fn pliego_pids(root: &Path) -> Vec<u32> {
    let root = root.display().to_string().to_lowercase();
    let mut pids = Vec::new();
    // SAFETY: recorrido estándar de Toolhelp con una instantánea propia que se cierra al final.
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return pids;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snap, &mut e).is_ok();
        while more {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..len]);
            if name.eq_ignore_ascii_case("pliego-app.exe")
                && image_path(e.th32ProcessID).is_some_and(|p| p.to_lowercase().starts_with(&root))
            {
                pids.push(e.th32ProcessID);
            }
            more = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    pids
}

unsafe extern "system" fn close_windows_of(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut pid = 0u32;
    // SAFETY: `hwnd` lo entrega EnumWindows; `pid` es un destino válido.
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == lparam.0 as u32 && IsWindowVisible(hwnd).as_bool() {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    BOOL(1)
}

/// Pide a las ventanas del proceso que se cierren (Pliego ofrece guardar los cambios).
pub fn post_close(pid: u32) {
    // SAFETY: la devolución de llamada solo usa su argumento `lparam` como número.
    unsafe {
        let _ = EnumWindows(Some(close_windows_of), LPARAM(pid as isize));
    }
}

fn known_folder(id: &windows::core::GUID) -> windows::core::Result<PathBuf> {
    // SAFETY: SHGetKnownFolderPath devuelve memoria de COM que se libera con CoTaskMemFree tras copiarla.
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None)?;
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const c_void));
        Ok(PathBuf::from(s))
    }
}

fn make_link(lnk: &Path, target: &Path) -> windows::core::Result<()> {
    if let Some(parent) = lnk.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let (t, wd, icon, path) = (
        wide(&target.display().to_string()),
        wide(&target.parent().unwrap_or(Path::new("")).display().to_string()),
        wide(&target.display().to_string()),
        wide(&lnk.display().to_string()),
    );
    // SAFETY: objetos COM creados y usados en este hilo; las cadenas terminan en cero y viven durante las llamadas.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(PCWSTR(t.as_ptr()))?;
        link.SetWorkingDirectory(PCWSTR(wd.as_ptr()))?;
        link.SetDescription(windows::core::w!("Pliego"))?;
        link.SetIconLocation(PCWSTR(icon.as_ptr()), 0)?;
        let store: IPropertyStore = link.cast()?;
        let id = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::from("Pliego.App");
        store.SetValue(&PKEY_AppUserModel_ID, &id)?;
        store.Commit()?;
        let file: IPersistFile = link.cast()?;
        file.Save(PCWSTR(path.as_ptr()), true)
    }
}

pub fn create_shortcuts(scope: Scope, launcher: &Path, desktop: bool) -> windows::core::Result<Vec<PathBuf>> {
    // SAFETY: inicialización de COM del hilo; si ya estaba inicializado en otro modo se sigue igualmente.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    let (programs, desk) = match scope {
        Scope::User => (&FOLDERID_Programs, &FOLDERID_Desktop),
        Scope::Machine => (&FOLDERID_CommonPrograms, &FOLDERID_PublicDesktop),
    };
    let mut out = vec![known_folder(programs)?.join("Pliego.lnk")];
    if desktop {
        out.push(known_folder(desk)?.join("Pliego.lnk"));
    }
    for lnk in &out {
        make_link(lnk, launcher)?;
    }
    Ok(out)
}

pub fn broadcast_environment_change() {
    let env = wide("Environment");
    // SAFETY: `env` termina en cero y vive durante la llamada, que es síncrona.
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(env.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
    }
}

pub fn notify_assoc_changed() {
    // SAFETY: sin punteros: avisa al Explorador de que cambiaron las asociaciones.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    refresh_icon_cache();
}

/// Pide a Windows que vuelva a leer los iconos (los del menú Inicio, el buscador y los `.pdf` asociados). Sin esto el
/// buscador de Windows llegó a enseñar el icono genérico durante horas tras instalar (2026-11-04). `ie4uinit -show`
/// existe desde Windows 10 y no necesita permisos; si falta o falla no pasa nada.
fn refresh_icon_cache() {
    let Some(system) = std::env::var_os("SystemRoot") else { return };
    let exe = Path::new(&system).join("System32").join("ie4uinit.exe");
    if !exe.is_file() {
        return;
    }
    match std::process::Command::new(&exe)
        .arg("-show")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        // No se espera: tarda poco, y el instalador no debe quedarse colgado por esto.
        Ok(_) => {}
        Err(e) => crate::logging::warn(&format!("ie4uinit -show: {e}")),
    }
}

pub fn open(what: &str) {
    let w = wide(what);
    // SAFETY: `w` termina en cero; ShellExecuteW copia lo que necesita.
    unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("open"),
            PCWSTR(w.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

pub fn relaunch_elevated(args: &[String]) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let quoted: Vec<String> = args.iter().map(|a| format!("\"{}\"", a.replace('"', "\\\""))).collect();
    let (e, p) = (wide(&exe.display().to_string()), wide(&quoted.join(" ")));
    // SAFETY: cadenas UTF-16 terminadas en cero válidas durante la llamada.
    let r = unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("runas"),
            PCWSTR(e.as_ptr()),
            PCWSTR(p.as_ptr()),
            None,
            SW_SHOWNORMAL,
        )
    };
    r.0 as usize > 32
}

pub fn pick_folder(start: &Path) -> Option<PathBuf> {
    // SAFETY: diálogo COM estándar usado en el hilo de la interfaz; todo se libera al salir del ámbito.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let opts = dlg.GetOptions().ok()?;
        dlg.SetOptions(opts | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM).ok()?;
        if let Ok(item) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(wide(&start.display().to_string()).as_ptr()), None)
        {
            let _ = dlg.SetFolder(&item);
        }
        dlg.Show(None).ok()?;
        let item = dlg.GetResult().ok()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const c_void));
        s.map(PathBuf::from)
    }
}

/// Copia este ejecutable a `%TEMP%` y lo lanza para que termine el trabajo cuando este proceso salga.
pub fn spawn_cleanup(root: &Path) {
    let Ok(me) = std::env::current_exe() else { return };
    let pid = std::process::id();
    let copy = std::env::temp_dir().join(format!("pliego-cleanup-{pid}.exe"));
    if std::fs::copy(&me, &copy).is_err() {
        return;
    }
    let _ = std::process::Command::new(&copy)
        .arg("--cleanup-from")
        .arg(root)
        .arg("--wait-pid")
        .arg(pid.to_string())
        .creation_flags(CREATE_NO_WINDOW | 0x0000_0008)
        .spawn();
}

/// Espera a que termine `pid` (hasta 60 s).
pub fn wait_for_exit(pid: u32) {
    if pid == 0 {
        std::thread::sleep(std::time::Duration::from_secs(2));
        return;
    }
    // SAFETY: se abre el proceso solo para esperarlo y se cierra el identificador.
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(h, 60_000);
            let _ = CloseHandle(h);
        }
    }
}

/// Borra este ejecutable (copia temporal) cuando termine el proceso.
pub fn delete_self_later() {
    if let Ok(me) = std::env::current_exe() {
        let _ = std::process::Command::new("cmd")
            .raw_arg(format!("/c ping -n 3 127.0.0.1 >nul & del /f /q \"{}\"", me.display()))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

/// Cuadro de error nativo para fallos anteriores a que exista la interfaz.
pub fn error_box(text: &str) {
    let (t, c) = (wide(text), wide("Pliego"));
    // SAFETY: cadenas UTF-16 terminadas en cero válidas durante la llamada; sin ventana propietaria.
    unsafe {
        MessageBoxW(None, PCWSTR(t.as_ptr()), PCWSTR(c.as_ptr()), MB_OK | MB_ICONERROR);
    }
}
