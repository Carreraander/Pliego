//! Detección e instalación de WebView2 (Evergreen Bootstrapper oficial de Microsoft, ver `instalador.md` §5).
#![allow(unsafe_code)]

use super::reg::{Hive, Key, wide};
use crate::i18n::{self, Lang};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, S_OK, WPARAM};
use windows::Win32::System::Com::{IBindStatusCallback, Urlmon::URLDownloadToFileW};
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::{IDOK, MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::{HRESULT, PCWSTR};

const GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const BOOTSTRAPPER_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

pub fn present() -> bool {
    let keys = [
        (
            Hive::LocalMachine,
            format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{GUID}"),
        ),
        (
            Hive::LocalMachine,
            format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{GUID}"),
        ),
        (
            Hive::CurrentUser,
            format!(r"Software\Microsoft\EdgeUpdate\Clients\{GUID}"),
        ),
    ];
    keys.iter().any(|(h, k)| {
        Key::open(*h, k)
            .and_then(|k| k.get_string("pv"))
            .is_some_and(|v| !v.is_empty() && v != "0.0.0.0")
    })
}

unsafe extern "system" fn task_callback(
    hwnd: HWND,
    msg: TASKDIALOG_NOTIFICATIONS,
    _: WPARAM,
    _: LPARAM,
    data: isize,
) -> HRESULT {
    // SAFETY: `data` es el puntero a `AtomicBool` que `ensure_webview2` mantiene vivo mientras dura el diálogo.
    let done = unsafe { &*(data as *const AtomicBool) };
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::SendMessageW;
        match msg {
            TDN_CREATED => {
                SendMessageW(
                    hwnd,
                    TDM_SET_PROGRESS_BAR_MARQUEE.0 as u32,
                    Some(WPARAM(1)),
                    Some(LPARAM(30)),
                );
            }
            TDN_TIMER if done.load(Ordering::Relaxed) => {
                SendMessageW(hwnd, TDM_CLICK_BUTTON.0 as u32, Some(WPARAM(IDOK.0 as usize)), None);
            }
            _ => {}
        }
    }
    S_OK
}

/// Si falta WebView2 lo descarga e instala en silencio con un `TaskDialog` nativo. `false` si no se pudo.
pub fn ensure_webview2(lang: Lang) -> bool {
    if present() {
        return true;
    }
    crate::logging::info("WebView2 no está instalado; se descarga el Evergreen Bootstrapper");
    let done = Arc::new(AtomicBool::new(false));
    let ok = Arc::new(AtomicBool::new(false));
    let worker = {
        let (done, ok) = (done.clone(), ok.clone());
        std::thread::spawn(move || {
            ok.store(download_and_run(), Ordering::Relaxed);
            done.store(true, Ordering::Relaxed);
        })
    };

    let title = wide("Pliego");
    let content = wide(i18n::webview2_title(lang));
    let cfg = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_SHOW_MARQUEE_PROGRESS_BAR | TDF_CALLBACK_TIMER | TDF_ALLOW_DIALOG_CANCELLATION,
        dwCommonButtons: TDCBF_CANCEL_BUTTON,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszContent: PCWSTR(content.as_ptr()),
        pfCallback: Some(task_callback),
        lpCallbackData: Arc::as_ptr(&done) as isize,
        ..Default::default()
    };
    let mut pressed = 0i32;
    // SAFETY: `cfg` y los búferes de texto viven hasta que el diálogo se cierra; `done` también (lo guarda `Arc`).
    let r = unsafe { TaskDialogIndirect(&cfg, Some(&mut pressed), None, None) };
    if r.is_err() {
        // Sin Common-Controls 6 (no debería pasar con el manifiesto): se espera sin diálogo.
        let _ = worker.join();
    } else if pressed != IDOK.0 {
        crate::logging::warn("instalación de WebView2 cancelada");
        return false;
    } else {
        let _ = worker.join();
    }
    let good = ok.load(Ordering::Relaxed) && present();
    if !good {
        let (t, c) = (wide("Pliego"), wide(i18n::webview2_offline(lang)));
        // SAFETY: cadenas UTF-16 terminadas en cero válidas durante la llamada.
        unsafe {
            MessageBoxW(None, PCWSTR(c.as_ptr()), PCWSTR(t.as_ptr()), MB_OK | MB_ICONERROR);
        }
    }
    good
}

fn download_and_run() -> bool {
    let path: PathBuf = std::env::temp_dir().join("MicrosoftEdgeWebview2Setup.exe");
    let (url, dest) = (wide(BOOTSTRAPPER_URL), wide(&path.display().to_string()));
    // SAFETY: cadenas UTF-16 terminadas en cero válidas durante la llamada.
    let r = unsafe {
        URLDownloadToFileW(
            None::<&windows::core::IUnknown>,
            PCWSTR(url.as_ptr()),
            PCWSTR(dest.as_ptr()),
            0,
            None::<&IBindStatusCallback>,
        )
    };
    if r.is_err() {
        crate::logging::error(&format!("descarga de WebView2: {r:?}"));
        return false;
    }
    use std::os::windows::process::CommandExt;
    let status = std::process::Command::new(&path)
        .args(["/silent", "/install"])
        .creation_flags(0x0800_0000)
        .status();
    let _ = std::fs::remove_file(&path);
    matches!(status, Ok(s) if s.success())
}
