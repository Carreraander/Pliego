# Lecciones técnicas ya aprendidas

Trampas que costó descubrir. Consultar antes de tocar lo relacionado.

## mupdf-rs 0.8.0
- `Document::outlines()` hace `strlen(NULL)` (SEGV) con un marcador sin `/Title`. Se esquiva con un recorrido propio
  en `crates/pliego-engine-mupdf/src/outline.rs`. Borrador de issue: `borradores/w02-issue-mupdf-rs-outline-null-title.md`.
- `PdfDocument::open` solo acepta `&str` (usar `to_str()`). MuPDF mantiene el archivo abierto: en Windows bloquea el
  rename encima. En la CLI, abrir leyendo a memoria (`PdfDocument::from_bytes`) o `--in-place` falla con «Access denied».
- `max_align_t` al compilar mupdf-sys en Windows: falta `clang.exe` en el PATH para bindgen (`acr12-env.ps1` lo arregla).
  Arreglado aguas arriba (issues #222 y #258) pero sin publicar.
- No expone `pdf_subset_fonts` (borrador `borradores/w08-propuesta-mupdf-rs-subset-fuentes.md`), ni nombres ni render
  por plancha de `Separations` (solo `len/active_count`), ni linealización.
- No genera `/AP` para campos nuevos: `pliego-formdesign/src/ap.rs` los dibuja a mano.
- mujs: `this.getField()` devuelve null de forma inestable; `recalculate` evalúa `/CO` en nativo. Validaciones sí
  funcionan con `doc.enable_js()` + `set_value(.., false)`.

## Windows
- `sync_all` exige handle de escritura.
- `windows-sys` 0.61: el job object necesita `Win32_Security` y `Win32_System_Threading` además de `JobObjects`.
  Carrera pendiente: el hijo arranca antes de entrar al job (lanzar con `CREATE_SUSPENDED`).
- `tests/corpus/.gitattributes` fija `*.pdf -text`: el CRLF de autocrlf rompe la xref. Igual en `docs/handoff/borradores/`.
- `con.pdf` (y `nul`, `aux`, `prn`...) son nombres reservados.
- Excel COM rechaza rutas `\\?\C:\...` de `canonicalize`: quitar el prefijo.
- El worker se lanza sin consola (`CREATE_NO_WINDOW`, #100); antes dejaba conhost colgado.
- WebView2 por sí solo ocupa ~300 MB en reposo.

## Cargo y CI
- El `Cargo.lock` de `main` no siempre incluye los crates de `app/` (Tauri); cualquier `cargo` puede reescribirlo
  (~5000 líneas). Commitear solo la entrada del crate nuevo y `git checkout -- Cargo.lock` antes de `git add -A` si el
  diff es enorme y no es tuyo.
- Linux: excluir `pliego-app`/`pliego-app-service` (GTK). `pliego-cli` necesita `libtesseract-dev libleptonica-dev`.
- La CI usa un clippy más nuevo que el local.
- La CI de Windows en verde no prueba el entorno de acr12 si reutiliza la caché.

## Módulos
- `pliego-actions` no depende de los módulos: la CLI registra los pasos en `crates/pliego-cli/src/commands/run.rs`
  (`builtin_registry`); un paso nuevo es `r.register("modulo.op", typed(...))`. Diario `.pliego-run.json` para `--resume`.
- Firma: pyHanko 0.38 (en un venv fuera del repo) valida B-B/B-T/B-LT en local. Certificados de prueba generados en los
  tests, nunca reales en el repo.
- Ghostscript 10.02: `-dPDFX` es booleano; la versión PDF/X va en `PDFX_def.ps`. No se incluyen perfiles ICC.
- OCR en Windows: Tesseract 5.5 y Leptonica 1.87 de vcpkg (`x64-windows-release`); ver `docs/ocr.md`.
