# W12 · PDF/A, PDF/UA y accesibilidad

Ola 2. Depende de: base existente.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w12` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w12-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Rebase sobre `main` antes de abrir el PR; regenera `Cargo.lock`, no lo resuelvas a mano.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-pdfa`, `crates/pliego-a11y`, `crates/pliego-cli/src/commands/{pdfa,a11y}.rs`.

## Objetivo
Convertir y validar PDF/A, comprobar PDF/UA y ayudar a hacer documentos accesibles.

## Alcance
- Convertir a PDF/A-1b, 2b y 3b (y 2u si es viable): incrustar fuentes, perfil de salida ICC sRGB, metadatos XMP, quitar transparencia o JS cuando el nivel lo exija, adjuntos solo en 3b.
- Validación propia de las reglas más comunes y validación completa opcional con veraPDF (descarga aparte, Java) con informe legible.
- Comprobador de accesibilidad: etiquetas, orden de lectura, texto alternativo, idioma del documento, título, contraste básico; informe con correcciones sugeridas.
- Edición de etiquetas y texto alternativo; etiquetado automático inicial con heurísticas (encabezados por tamaño, listas, tablas simples).

## Interfaces que debes exponer
- `pliego_pdfa::{convert(input, out, Level, ctx) -> Report, validate(input, Level) -> Validation}`.
- `pliego_a11y::{check(doc) -> A11yReport, set_alt_text, set_language, autotag(ctx)}`.

## Criterios de "hecho"
- Los PDF/A generados pasan veraPDF sin errores para 1b, 2b y 3b en el corpus de prueba (en CI o en acr12).
- El comprobador detecta los fallos de un conjunto de PDFs de prueba con defectos conocidos.
