# W02 · Motor, aislamiento y guardado

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w02` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w02-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Para actualizar la rama, haz merge de `main` en ella (nunca rebase ni force-push); regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-engine-mupdf`, `crates/pliego-worker` (nuevo), `crates/pliego-document` (nuevo).

## Objetivo
Hacer el motor robusto para producción: PDFs hostiles en proceso aislado, caché de teselas, modelo de documento editable con revisiones, deshacer y guardado seguro.

## Alcance
- `pliego-worker`: proceso hijo que abre documentos con MuPDF y responde por stdin/stdout (mensajes longitud+JSON, binarios para imágenes). Reinicio automático si cae. Privilegios reducidos en Windows (job object con límite de memoria; restricted token si es viable).
- Cliente `WorkerEngine` que implementa `PdfEngine`/`EngineDocument` hablando con el worker, intercambiable con `MupdfEngine`.
- Caché LRU de teselas por (doc, revisión, página, zoom, col, row) con límite de memoria configurable.
- `pliego-document`: `Document` editable (envuelve `mupdf::pdf::PdfDocument`) con número de revisión, pila de deshacer/rehacer basada en instantáneas en memoria o en guardado incremental, `save(path, SaveOptions)` atómico (temporal + renombrar), guardado incremental cuando el documento está firmado, reparación de PDFs dañados.
- Fuzzing del parser con `cargo-fuzz` (objetivos: abrir, render de página 0, extraer texto).

## Interfaces que debes exponer
- `pliego_document::Document { open, revision(), with_pdf_mut(|pdf| ...) -> Result<T>, undo(), redo(), save(), save_incremental() }`. Es la API que usará la app para aplicar operaciones de módulos.
- `pliego_worker::WorkerEngine` (implementa los traits de `pliego-core`).
- Cambios en `pliego-core` (si hacen falta) en un PR separado y pequeño, avisando al hilo de gestión.

## Criterios de "hecho"
- Un PDF corrupto o malicioso del corpus de fuzzing no tumba el proceso principal (test que mata el worker y verifica la recuperación).
- 1 hora de fuzzing sin fallos nuevos en CI nocturna o en acr12; fallos encontrados corregidos o reportados.
- Deshacer/rehacer y guardado atómico cubiertos por tests; guardar un PDF firmado no invalida la firma (incremental).
- Rendimiento: render de tesela 512 px en < 30 ms de media en acr12 para páginas de texto.
