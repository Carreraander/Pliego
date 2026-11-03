# W06 · OCR

Ola 1. Depende de: base existente.

## Contexto (léelo entero)
Eres una sesión de implementación del proyecto **Pliego**: una alternativa libre (AGPL-3.0-or-later) y gratuita a
Adobe Acrobat Pro para Windows. Ander es el dueño del proyecto. Hay un hilo de gestión ("Diseñar alternativa open
source a Acrobat Pro", id `cmsg_01VzNAbDDgN8F4HKcyc76pSD2RqHEmBxL1s12gPvzmcf2G`) que coordina todos los flujos, revisa
PRs y decide sobre interfaces compartidas. Escríbele con `message_thread` usando ese id.

- Repositorio: https://github.com/Carreraander/Pliego (rama `main`). Añádelo a tu sesión y clónalo.
- Diseño completo: https://claude.ai/artifact/Wu4ZXyGqToREgugipQuNgw
- Lee antes de empezar: `CLAUDE.md`, `docs/CONVENTIONS.md`, `docs/ARCHITECTURE.md`.
- Stack: Rust (edición 2024) + MuPDF vía crate `mupdf` 0.8 (AGPL) · Tauri 2 + WebView2 + SolidJS · CLI `pliego`.
- Ya existe: `pliego-core` (Error, PageRange, JobContext, Report, trait PdfEngine), `pliego-engine-mupdf`
  (render de página/tesela, texto, búsqueda, marcadores) y `pliego-cli` (info, render, text, search).
- Equipo de pruebas Windows: usuario `acr12` en `DESKTOP-41TJN0V`. Para compilar o probar allí, pide una sesión
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w06` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w06-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Para actualizar la rama, haz merge de `main` en ella (nunca rebase ni force-push); regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-ocr`, `crates/tesseract-sys` si hace falta, `crates/pliego-cli/src/commands/ocr.rs`, `tools/` para scripts de construcción de Tesseract.

## Objetivo
Convertir escaneos en PDFs con texto buscable y seleccionable, con calidad cercana a OCRmyPDF.

## Alcance
- Integración con Tesseract 5 (Apache-2.0) mediante su API C. En Windows, construcción reproducible (vcpkg o script propio) y DLLs empaquetables; en Linux, librería del sistema para CI.
- Preprocesado: detección de orientación y rotación, enderezado (deskew), binarización adaptativa y eliminación de ruido opcionales, resolución mínima 300 ppp para el reconocimiento.
- Capa de texto invisible alineada palabra a palabra sobre la imagen original (sin recomprimir la imagen salvo que se pida).
- Modos: saltar páginas que ya tienen texto, forzar, solo páginas sin texto. Idiomas múltiples (`spa+eng`). Paralelismo por página con límite de hilos.
- Gestor de modelos `tessdata_fast`: descarga bajo demanda a `%LOCALAPPDATA%\Pliego\tessdata`, con verificación de hash. `spa` y `eng` incluidos en el instalador.
- Diseño con trait `OcrEngine` para añadir PaddleOCR/ONNX más adelante.

## Interfaces que debes exponer
- `pliego_ocr::{ocr_document(input, out, OcrOptions, ctx) -> Report, ocr_page_image(img, langs) -> Vec<Word>, available_languages(), download_language(code, ctx)}`.
- `trait OcrEngine` + `TesseractEngine`.
- CLI: `pliego ocr entrada.pdf -o salida.pdf --lang spa+eng --skip-text|--force --deskew`.

## Criterios de "hecho"
- Sobre un corpus de 10 escaneos generados (texto conocido renderizado, rotado y con ruido), tasa de error por carácter < 3 % en `spa`.
- La búsqueda y la selección de texto funcionan en el PDF resultante en Acrobat Reader y en el visor de Pliego.
- 20 páginas a 300 ppp en < 60 s en acr12.
- Script reproducible para compilar o empaquetar Tesseract en Windows, documentado.
