# Equipo de pruebas acr12

Equipo Windows de pruebas del proyecto: `DESKTOP-41TJN0V`, usuario `acr12`. Cada flujo trabaja en `C:\Users\acr12\dev\pliego-<flujo>`.

## Preparación

```powershell
cd C:\Users\acr12\dev\pliego-<flujo>
powershell -ExecutionPolicy Bypass -File tools\setup-acr12.ps1 -Report acr12-setup.md
```

El script es idempotente (instala lo que falte con winget y cargo; `-CheckOnly` solo verifica) y devuelve código 1 si falta algo.

## Primera ejecución (2026-11-03)

- Windows: Microsoft Windows 10 Pro 10.0.19045
- `CARGO_HOME`: `D:\dev-cache\cargo-home` (el script lo respeta)

| Herramienta | Estado | Versión |
|---|---|---|
| Git | ya estaba | 2.53.0.windows.1 |
| Node LTS | ya estaba | v24.16.0 |
| LLVM (clang) | ya estaba | 23.1.4 |
| WiX Toolset | instalado | 3.14.1.8722 (activó la característica de Windows NetFx3) |
| VS Build Tools C++ | ya estaba | VS 17.13.35919.96 |
| Rust (rustc / cargo) | ya estaba | 1.96.0 |
| cargo-deny | instalado (compilación ~6 min) | 0.20.2 |

## Resultados de comprobaciones y rendimiento

- Segunda pasada de `setup-acr12.ps1`: todo "ya estaba", código de salida 0 (idempotente).
- `cargo fmt --all --check`: correcto.
- `cargo clippy --workspace --all-targets -- -D warnings`: **falla** al compilar `mupdf` 0.8.0 con
  `error[E0425]: cannot find type max_align_t` (`mupdf-0.8.0\src\device\native.rs:599`).
  Causa probable: con el objetivo MSVC y clang 23, `bindgen` no genera `max_align_t` (clang solo lo define en modo C11 o superior).
  Es un fallo del entorno de acr12 (la CI de Windows pasa con el clang del runner), no del código de Pliego.
  Soluciones por probar: `$env:BINDGEN_EXTRA_CLANG_ARGS="-std=c11"` con recompilación limpia de `mupdf-sys`, o instalar un LLVM más antiguo.
- `cargo test --workspace`: pendiente hasta resolver lo anterior.
- `CARGO_TARGET_DIR` global en acr12 apunta a `D:\dev-cache\cargo-target`. Cada flujo usa `. .\tools\acr12-env.ps1` para tener el suyo en `D:\dev-cache\target\<clon>` y LLVM 19 en `D:\LLVM19` (C: tiene poco espacio).
- Benchmarks por versión: pendiente.

## Resultados con el entorno completo (2026-11-03, clon en `1c9fb8a`)

Entorno: `. .\tools\acr12-env.ps1` (LLVM 19 en el PATH, Tesseract 5.5/Leptonica 1.87 de vcpkg). Espacio libre: C: 45 GB, D: 518 GB; el `target` de un clon ocupa ~15 GB.

- `mupdf-sys` compila. Causa real del `max_align_t`: faltaba `clang.exe` en el PATH para bindgen (no es la versión de LLVM). Arreglado
  upstream en mupdf-rs (issues #222 y #258) pero sin publicar (última: 0.8.0). La CI de Windows en verde no lo prueba si reutiliza la caché.
- `cargo fmt --all --check`: correcto.
- `cargo clippy ... -D warnings`: **4 errores `nonminimal_bool`** (clippy 1.96): `pliego-formdesign/src/detect.rs:622` (2),
  `pliego-convert/src/images.rs:249`, `pliego-convert/src/text.rs:75`.
- `cargo test --workspace`: 467 correctos, **8 fallos** solo en Windows:
  - `pliego-a11y/tests/autotag.rs` (2): el archivo `con.pdf` usa el nombre de dispositivo reservado CON. Renombrar.
  - `pliego-signflow/tests/flow.rs` (3) y `pliego-cli/tests/signflow.rs` (1): "PDF no válido para firmar: object ID 4 0 not found".
  - `pliego-convert/tests/office.rs` (1): Excel COM rechaza la ruta `\\?\C:\...` (prefijo de `canonicalize`); quitar el prefijo.
  - `pliego-cli/tests/convert.rs` (1): espera "LibreOffice" en el error, pero en acr12 se usa el backend Excel.

## Espacio en disco (C: casi lleno)

C: se quedó sin espacio. Reglas desde ahora: clones nuevos en `D:\dev\pliego-<flujo>` (`setup-acr12.ps1` ya usa `D:\dev` por defecto);
`. .\tools\acr12-env.ps1` mueve a D: el `target` (por clon), `TEMP`/`TMP`, la caché de npm, los navegadores de Playwright y la caché
binaria de vcpkg. No borrar carpetas de otros proyectos de acr12; solo `target` y `node_modules` regenerables de clones `pliego-*`.
