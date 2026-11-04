//! Job object de Windows para el worker: límite de memoria, un solo proceso y muerte con la app.
//!
//! No se usa *restricted token*: no es viable de forma portable con `std::process::Command` sin
//! reimplementar `CreateProcessAsUser`; el job object ya impide crear procesos hijos y limita memoria.
#![allow(unsafe_code)]

use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::io::AsRawHandle;
use std::process::Child;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

/// `CREATE_SUSPENDED`: el hijo nace parado y no ejecuta nada hasta que `attach` lo mete en el job.
/// Así no hay ventana en la que el worker corra sin límites (ni pueda crear procesos antes de ser asignado).
pub const CREATE_SUSPENDED: u32 = 0x0000_0004;

/// Dueño del job object. Al soltarlo se cierra el handle y, por `KILL_ON_JOB_CLOSE`, muere el worker.
pub struct Sandbox {
    job: HANDLE,
}

// SAFETY: un HANDLE de job object es un identificador del núcleo, válido desde cualquier hilo.
unsafe impl Send for Sandbox {}

impl Sandbox {
    /// Mete `child` en un job nuevo con los límites indicados y lo reanuda.
    ///
    /// `child` debe haberse lanzado con [`CREATE_SUSPENDED`]; si falla algo, queda parado y el llamador lo mata.
    pub fn attach(child: &Child, memory_limit: Option<usize>) -> io::Result<Option<Self>> {
        // SAFETY: se pasan punteros nulos permitidos (atributos de seguridad y nombre opcionales).
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let sandbox = Sandbox { job }; // si algo falla, Drop cierra el handle

        // SAFETY: la estructura es de datos planos y el valor cero es válido (sin límites activos).
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        let mut flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        info.BasicLimitInformation.ActiveProcessLimit = 1;
        if let Some(bytes) = memory_limit {
            flags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            info.ProcessMemoryLimit = bytes;
        }
        info.BasicLimitInformation.LimitFlags = flags;

        // SAFETY: `job` es válido, `info` vive durante la llamada y el tamaño coincide con la clase indicada.
        let ok = unsafe {
            SetInformationJobObject(
                sandbox.job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: ambos handles son válidos; el del proceso lo mantiene vivo `child`.
        let ok = unsafe { AssignProcessToJobObject(sandbox.job, child.as_raw_handle() as HANDLE) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        resume_main_thread(child.id())?;
        Ok(Some(sandbox))
    }
}

/// Reanuda el hilo principal de un proceso creado suspendido (`std::process::Child` no expone su handle).
fn resume_main_thread(pid: u32) -> io::Result<()> {
    // SAFETY: instantánea de hilos del sistema; el handle devuelto se cierra siempre más abajo.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut resumed = false;
    // SAFETY: `snap` es válido y `entry` tiene `dwSize` inicializado, como exige la API.
    let mut more = unsafe { Thread32First(snap, &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: se abre el hilo solo con permiso de suspender/reanudar y se cierra tras usarlo.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if !thread.is_null() {
                // SAFETY: `thread` es válido; ResumeThread devuelve u32::MAX si falla.
                resumed = unsafe { ResumeThread(thread) } != u32::MAX;
                // SAFETY: handle abierto arriba y cerrado una sola vez.
                unsafe { CloseHandle(thread) };
            }
            break; // un proceso recién creado y suspendido solo tiene su hilo principal
        }
        // SAFETY: igual que en `Thread32First`.
        more = unsafe { Thread32Next(snap, &mut entry) } != 0;
    }
    // SAFETY: `snap` se creó arriba y se cierra una sola vez.
    unsafe { CloseHandle(snap) };
    if resumed {
        Ok(())
    } else {
        Err(io::Error::other("no se pudo reanudar el worker suspendido"))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // SAFETY: el handle se creó en `attach` y solo se cierra aquí, una vez.
        unsafe { CloseHandle(self.job) };
    }
}
