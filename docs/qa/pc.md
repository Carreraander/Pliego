# Equipo de pruebas «pc»

Segundo equipo Windows de pruebas del proyecto (el principal es acr12, ver [`acr12.md`](acr12.md)).

- `DESKTOP-TBC7MHI`, usuario `pc`, **Windows 11 Home** 10.0.26200 (con Mica), 32 GB de RAM, 8 hilos.
- **Un solo disco**: C: de 256 GB, con **muy poco sitio libre** (10,7 GB el 2026-11-04 antes de preparar nada). No hay D:.
- Las sesiones de Remote Control en `pc` van **elevadas** (nivel obligatorio alto, sin UAC; comprobado el 2026-11-05): se puede
  instalar en todo el equipo y crear tarjetas inteligentes virtuales. Aun así, nada fuera de Pliego.
- Hay **TPM 2.0** (Nuvoton) y `tpmvscmgr.exe`: permite probar el proveedor de tarjeta inteligente de Windows (como el DNIe)
  sin lector. Hay un lector Broadcom integrado, sin tarjeta.
- Escritorio interactivo disponible: `CopyFromScreen` y UI Automation funcionan, pero **desde un `.ps1` en disco**: el antivirus
  (AMSI) bloquea el código de captura lanzado en línea. No hay `quser` (Windows 11 Home). Las capturas pueden enseñar el
  escritorio real de Ander: borrarlas al acabar.
- En el equipo hay otros proyectos (p. ej. `TraceholdGit`, `Tracehold`, `vm-main`...): **no son de Pliego, no se tocan**.

## Rutas de Pliego en pc

| Qué | Ruta |
|---|---|
| Clones | `C:\dev\pliego-<flujo>` (p. ej. `C:\dev\pliego-w10`) |
| Herramientas propias | `C:\dev-tools` (vcpkg; LLVM 19, JRE y veraPDF si se instalan) |
| Target, temporales, npm, Playwright, vcpkg | `C:\dev-cache` (`target\<clon>`, `tmp`, `npm`, `ms-playwright`, `vcpkg-binary`, `vcpkg-downloads`) |
| Turno | `C:\dev-cache\turnos\turno.txt` |
| Tesseract/Leptonica | `C:\dev-tools\vcpkg\installed\x64-windows-release` |

## Scripts

Los scripts de acr12 se han generalizado; los de acr12 siguen funcionando igual (son envoltorios):

| Genérico | acr12 | pc |
|---|---|---|
| `tools\setup-windows.ps1` (parámetros `-Disco`, `-DevRoot`, `-ToolsRoot`, `-Llvm19Dir`, `-SinLlvm19`, `-ConTesseract`, `-ConExtras`, `-CheckOnly`, `-Report`) | `tools\setup-acr12.ps1` | ver abajo |
| `tools\pliego-env.ps1` (detecta D:; si no hay, C:) | `tools\acr12-env.ps1` | `tools\pc-env.ps1` |
| `tools\pliego-turno.ps1` (mutex `Global\PliegoAcr12TurnoUnico`, FIFO, código 75 si se agota la espera) | `tools\acr12-turno.ps1` | `tools\pc-turno.ps1` |

Preparar o verificar pc (idempotente):

```powershell
cd C:\dev\pliego-<flujo>
powershell -ExecutionPolicy Bypass -File tools\setup-windows.ps1 -Disco C: -SinLlvm19 -ConTesseract -ConExtras -Report pc-setup.md
```

`pliego-env.ps1` también admite `$env:PLIEGO_CACHE` (carpeta de cachés) y `$env:PLIEGO_LLVM` (carpeta `bin` de LLVM)
para forzar otras rutas.

## Reglas de compilación en pc (obligatorias)

1. **Todo en `C:\dev`, `C:\dev-tools` y `C:\dev-cache`.** Nada fuera de ahí, y nunca carpetas de otros proyectos.
2. **Cada flujo, su propio clon** (pero el `target` es compartido; el turno único evita bloqueos) `C:\dev\pliego-<flujo>`. Solo se pueden borrar `target` y `node_modules`
   regenerables de clones `pliego-*` (y `C:\dev-cache\target\pliego-*`).
3. **En cada terminal:** `. .\tools\pc-env.ps1` (con punto). Pone `clang.exe` en el PATH (LLVM del sistema si no hay
   LLVM 19), `CARGO_TARGET_DIR=C:\dev-cache\target\compartido` (un solo target para todos los flujos de pc, no uno por clon), temporales y cachés en `C:\dev-cache`, Tesseract y
   `CARGO_BUILD_JOBS=4`. Por el poco disco, en C: además pone `CARGO_INCREMENTAL=0` y `CARGO_PROFILE_DEV_DEBUG=0`
   (sin información de depuración: si hace falta depurar, ponla a mano en esa terminal).
4. **Un solo turno**, con el mismo mutex que acr12: `.\tools\pc-turno.ps1 -Flujo wNN -- <orden>`.
5. **Mirar el disco antes de compilar.** Con menos de ~4 GB libres no se lanza nada pesado: se borra primero el
   `target` propio. Nada de `cargo build --workspace` completo ni `--release` de toda la app salvo que haya sitio
   (un target completo ocupa ~15 GB en acr12).
6. Lo que no termina (`tauri dev`, servidores) va fuera del turno. Cerrar lo propio al acabar.

## Límite de uso de pc

Con ~8 GB libres en C: **no cabe un target de Rust completo (más de 10 GB) ni el modelo de llama de W18**. Por ahora
la segunda cola de pc solo vale para tareas ligeras: veraPDF, la CLI en debug y las pruebas de un crate.

## Visual Studio 18 Insiders sin C++ (arreglado en `pliego-env.ps1`)

En pc hay dos Visual Studio: **VS 18 Insiders sin el componente C++** y **Build Tools 2022 (17.14) con C++**.
mupdf-sys usa siempre el MSBuild del VS más nuevo (crate `cc`, sin forma de elegir otro) y clang/bindgen también lo
prefiere, así que sin arreglo `cargo build` falla con `Microsoft.Cpp.Default.props was not found` y luego con
`'setjmp.h' file not found`. `pliego-env.ps1` lo detecta con `vswhere` (el VS más nuevo no tiene C++ pero otro sí) y,
solo en esa terminal:

- carga el entorno `vcvars64.bat` de Build Tools 2022 (INCLUDE/LIB para clang y `VisualStudioVersion=17.0`, con lo que
  mupdf-sys elige `PlatformToolset=v143`);
- escribe `C:\dev-cache\target\Directory.Build.rsp` con `/p:VsInstallRoot` y `/p:VCTargetsPath` de Build Tools 2022.
  MSBuild lee ese fichero solo para lo que se compila bajo `C:\dev-cache\target`; no afecta a otros proyectos.

Si algún día se instala C++ en VS 18 (o se quita VS 18), el script deja de hacerlo y borra el `.rsp`.

## Estado de la preparación (2026-11-04)

| Herramienta | Estado | Versión / ruta |
|---|---|---|
| Git | ya estaba | 2.55.0.windows.2 |
| Node LTS | ya estaba | 24.18.0 |
| LLVM (clang) | ya estaba | 22.1.8 (`C:\Program Files\LLVM`) |
| LLVM 19 | **no instalado** (`-SinLlvm19`) | no hace falta: compila con LLVM 22 y `clang.exe` en el PATH; ocuparía ~2,5 GB |
| VS Build Tools C++ | ya estaba | 2022, 17.14 (MSVC 14.44) |
| Rust | ya estaba | 1.97.0 |
| cargo-deny | ya estaba | 0.20.2 |
| WiX | ya estaba | 5.0.2 |
| Tesseract/Leptonica | ya estaba | vcpkg en `C:\dev-tools\vcpkg\installed\x64-windows-release` |
| veraPDF | instalado | 1.30.3 en `C:\dev-tools\veraPDF`, con JRE 21 portátil en `C:\dev-tools\jre` |
| Acrobat Reader | instalado (UAC aceptado por Ander) | winget `Adobe.Acrobat.Reader.64-bit`, en `C:\Program Files\Adobe\Acrobat DC` |
| Ghostscript | instalado (UAC aceptado por Ander) | 10.08.0 en `C:\Program Files\gs`; ya no está en winget: instalador firmado de Artifex (GitHub) |
| Office | no se instala | regla del proyecto |

Comprobación: `.\tools\pc-turno.ps1 -Flujo w10 -- cargo build -p pliego-cli` desde cero compila en **~6 min**
(mupdf-sys ~4,5 min). El target de `pliego-w10` ocupa **3,5 GB** (sin información de depuración ni incremental).
Lo que se instala en todo el equipo (Reader, Ghostscript) pide UAC: se lanza con Ander delante de la pantalla.
