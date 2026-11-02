# W10 · Build, QA e integración con Windows

Ola 1. Depende de: base existente; el instalador necesita W01.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w10` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w10-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Rebase sobre `main` antes de abrir el PR; regenera `Cargo.lock`, no lo resuelvas a mano.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`.github/`, `tools/`, `fuzz/`, `installer/`, `tests/corpus/README.md`, `benches/` (nuevo), `crates/pliego-shell` (nuevo, integración con el Explorador).

## Objetivo
Que el proyecto se compile, pruebe, mida y distribuya de forma fiable en Windows, y que Pliego se integre con el sistema como visor PDF.

## Alcance
- Script `tools/setup-acr12.ps1` idempotente que instala o verifica Rust, VS Build Tools (C++), LLVM, Node LTS, Git, WiX y cargo-deny en acr12. Ejecútalo primero en acr12 y documenta el resultado: los demás flujos lo reutilizarán.
- CI: caché, artefactos de la CLI para Windows, job nocturno de fuzzing (con W02) y de corpus amplio, `cargo deny` completo.
- Corpus: `tools/fetch_corpus` que descarga suites públicas (pdf.js test PDFs, veraPDF corpus, Isartor) a `tests/corpus/external/` (ignorado por git) y un test que abre y renderiza todo sin fallos ni bloqueos (timeout por archivo).
- Benchmarks reproducibles (`benches/` o `tools/bench`) para los objetivos de rendimiento, ejecutables en acr12.
- `crates/pliego-shell`: asociación de .pdf, entradas del menú contextual del Explorador (Combinar con Pliego, Comprimir, OCR, Proteger) y controlador de vista previa del Explorador (preview handler) si es viable.
- Instalador: MSI con WiX y versión portable ZIP; MSIX después. Firma de código preparada (sin certificado real todavía). Actualizaciones: diseño documentado.

## Interfaces que debes exponer
- `tools/setup-acr12.ps1`, `tools/fetch_corpus.(ps1|py)`, `installer/README.md` con cómo generar el instalador.
- Informe `docs/qa/acr12.md` con versión de Windows, herramientas instaladas y resultados de rendimiento por versión.

## Criterios de "hecho"
- `tools/setup-acr12.ps1` ejecutado con éxito en acr12 y documentado (primer PR, prioritario).
- Test de corpus externo: 0 bloqueos y 0 caídas en las suites descargadas (fallos de render de archivos rotos aceptados y listados).
- Instalador MSI que instala app + CLI, asocia .pdf y desinstala limpio en acr12 (cuando W01 tenga app).
- CI verde en Windows y Linux en `main`.
