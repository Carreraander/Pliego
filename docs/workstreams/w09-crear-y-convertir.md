# W09 · Crear y convertir

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w09` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w09-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Rebase sobre `main` antes de abrir el PR; regenera `Cargo.lock`, no lo resuelvas a mano.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-convert`, `crates/pliego-cli/src/commands/convert.rs`.

## Objetivo
Crear PDFs desde otros formatos y exportar PDFs a formatos comunes. La exportación a Word/Excel/PowerPoint de calidad es W14 (ola 2); aquí se deja la base.

## Alcance
- Crear PDF: desde imágenes (JPG, PNG, TIFF multipágina, WebP, HEIC si es viable) con tamaño de página y márgenes; desde texto plano y Markdown; desde HTML (en Windows, impresión a PDF de WebView2; documentar alternativa en Linux).
- Office a PDF: en Windows, automatización COM de Word/Excel/PowerPoint si están instalados (`#[cfg(windows)]`, `src/platform/windows.rs`); si no, LibreOffice en modo headless si está instalado (detección de ruta). Mensaje claro si no hay ninguno.
- Exportar: PDF a PNG/JPEG/TIFF (por página, ppp configurable), a texto, a HTML (texto estructurado de MuPDF), a SVG por página, extraer imágenes incrustadas en su formato original.
- Base para W14: función que devuelve el texto estructurado (bloques, líneas, spans con fuente, tamaño, color, bbox) serializable.

## Interfaces que debes exponer
- `pliego_convert::{images_to_pdf, text_to_pdf, html_to_pdf, office_to_pdf, pdf_to_images, pdf_to_text, pdf_to_html, pdf_to_svg, extract_images, structured_text(doc, page) -> StructuredPage}`.
- `fn office_backend() -> Option<OfficeBackend>` para que la app sepa qué está disponible.
- CLI: `pliego create <archivos...> -o salida.pdf`, `pliego export entrada.pdf --to png|jpg|tiff|txt|html|svg|images -o carpeta`.

## Criterios de "hecho"
- Tests de ida y vuelta: imágenes a PDF y de vuelta a imágenes con mismas dimensiones; texto exportado contiene el texto original.
- En acr12: Office a PDF probado con Word si está instalado, o con LibreOffice instalado para la prueba (captura y nota en el PR).
- `structured_text` documentado y con test, listo para W14.
