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
- Benchmarks por versión: ver «Benchmarks del motor» más abajo.

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

### Limpieza del 2026-11-03
C: pasó de 28,8 GB a 41,2 GB libres (12,4 GB liberados): borrados los `target` de clones en C: (pliego-w06 10,5 GB, pliego-w18 0,75 GB, pliego-w11 0,4 GB),
temporales de Playwright y `%LOCALAPPDATA%\ms-playwright`. Ocupaban sitio y siguen en C: (no se tocan): `.rustup` 4,4 GB, `vcpkg` 3 GB,
`%TEMP%` 5,9 GB (ajeno a Pliego), `.cargo` 0,3 GB; modelos GGUF de pliego-w18 (5,6 GB) pendientes de decisión de Ander.
Los temporales, `target`, npm y Playwright de ahora en adelante van a D: con `. .\\tools\\acr12-env.ps1`.

## Benchmarks del motor (2026-11-04, rama del PR #93 en `d05ebc1`, versión 0.1.0)

`tools\bench.ps1` por turno (`acr12-turno.ps1 -Flujo w10`), 3 ejecuciones seguidas con `bench.exe` en release
(compilado en 2 min 57 s por turno). PDF: `tests\output\bench-500.pdf` (500 páginas de texto, Helvetica, 2,3 MB).
Sin compilaciones ni otros benchmarks en marcha y con la CPU casi en reposo, pero con poca RAM libre (sesiones de
Claude, Chrome y otras apps abiertas). Cifras de la pasada `--json`, que es la que mide la memoria:

| Ejecución | RAM libre antes | CPU antes | Abrir mediana / p95 | Tesela 512 media / p95 | Página 2x | Memoria máxima |
|---|---|---|---|---|---|---|
| 1 | 2 948 MB | 1 % | 0,56 / 0,61 ms | 16,2 / 2,2 ms | 8,8 ms | 39,9 MB |
| 2 | 3 380 MB | 20 % | 0,54 / 0,61 ms | 18,9 / 5,5 ms | 12,9 ms | 46,5 MB |
| 3 | 3 252 MB | 7 % | 0,61 / 0,75 ms | 17,0 / 2,2 ms | 7,9 ms | 47,3 MB |

Objetivos (abrir < 1 500 ms, tesela 512 < 30 ms de media, memoria < 150 MB): **cumplidos** en las tres ejecuciones. La
pasada sin `--json` que imprime el script dio cifras parecidas (abrir 0,5-0,8 ms, tesela 16-21 ms, página 8-15 ms). El
JSON de la última ejecución queda en `docs/qa/bench/0.1.0-d05ebc1.json` (el script lo sobrescribe en cada ejecución).

**Ojo: la primera tesela en frío tarda ~1,5 s.** Por eso la media de tesela (16-19 ms) sale muy por encima del p95
(2-5 ms): con `bench.exe --tiles 1` la única tesela tarda 1 497 ms y 1 503 ms en dos ejecuciones, y las 99 siguientes,
unos 2 ms. Es un coste de arranque, probablemente la carga de la fuente Helvetica (no incrustada) o la inicialización
de MuPDF en el primer render. En la app sería un retraso de 1,5 s en la primera página que se ve: hay que investigarlo
(W02), precalentar el motor en segundo plano y medir aparte en `bench` la primera tesela y la media sin ella.

## Memoria de la app con WebView2 (2026-11-06, beta desde `main` en `f5d8cf9`)
`tools/bench-memoria-webview.ps1`, 3 repeticiones por variante, 25 s tras abrir `bench-100.pdf`, WebView2 156.0.4314.58. Misma
medición que la de la beta 0.1.0 (`docs/qa/memoria-webview/2026-11-06-0255-DESKTOP-41TJN0V.json`, 10 variantes); esta
(`docs/qa/memoria-webview/2026-11-06-0401-DESKTOP-41TJN0V.json`) repite 4 variantes con la beta nueva (incluye #213, #218,
#220 y #223).

| Variante | Procesos WebView2 | Memoria total (WS) | Privada total | Beta 0.1.0 (WS) |
|---|---|---|---|---|
| base | 6 | 452,5 MB | 192,4 MB | 453,2 MB |
| v8-lite | 6 | 423,9 MB | 167,0 MB | 423,1 MB |
| gpu-interna | 5 | 454,2 MB | 176,0 MB | 453,9 MB |
| combo-v8 | 5 | 453,3 MB | 176,4 MB | 456,8 MB |

Sin cambio de memoria respecto a la beta anterior: los cambios de `main` no la mueven. Solo `v8-lite` baja (unos 29 MB)
y las variantes `gpu-interna` y `combo-v8` ahorran un proceso y unos 16 MB de memoria privada, pero no de conjunto de trabajo.
Criterio de Ander (2026-11-06): memoria propia (app + worker) ≤ 150 MB, medida aparte (unos 52 MB: cumple), y cifra del
Administrador de tareas con WebView2 ≤ 500 MB (unos 452 MB de conjunto de trabajo: cumple). La configuración por defecto
de WebView2 se mantiene.
