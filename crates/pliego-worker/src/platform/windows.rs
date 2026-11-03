//! Job object de Windows para el worker: límite de memoria, un solo proceso y muerte con la app.
//!
//! No se usa *restricted token*: no es viable de forma portable con `std::process::Command` sin
//! reimplementar `CreateProcessAsUser`; el job object ya impide crear procesos hijos y limita memoria.
#![allow(unsafe_code)]

use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::io::AsRawHandle;
use std::process::Child;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

/// Dueño del job object. Al soltarlo se cierra el handle y, por `KILL_ON_JOB_CLOSE`, muere el worker.
pub struct Sandbox {
    job: HANDLE,
}

// SAFETY: un HANDLE de job object es un identificador del núcleo, válido desde cualquier hilo.
unsafe impl Send for Sandbox {}

impl Sandbox {
    /// Mete `child` en un job nuevo con los límites indicados.
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
        Ok(Some(sandbox))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // SAFETY: el handle se creó en `attach` y solo se cierra aquí, una vez.
        unsafe { CloseHandle(self.job) };
    }
}
