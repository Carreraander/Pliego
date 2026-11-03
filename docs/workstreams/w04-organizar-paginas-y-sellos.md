# W04 · Organizar páginas y sellos

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w04` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w04-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Para actualizar la rama, haz merge de `main` en ella (nunca rebase ni force-push); regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-organize`, `crates/pliego-stamp`, `crates/pliego-cli/src/commands/{pages,stamp}.rs`.

## Objetivo
Todas las operaciones de páginas y de marcado de documento: combinar, dividir, reordenar, y añadir encabezados, marcas de agua y numeración Bates.

## Alcance
- Combinar varios PDFs (conservando marcadores, enlaces internos y formularios con nombres de campo sin colisiones), dividir (cada N páginas, por rangos, por marcadores de nivel 1, por tamaño máximo), extraer, insertar (en blanco o de otro PDF), borrar, rotar, reordenar, duplicar, reemplazar páginas, recortar (CropBox) y etiquetas de página (PageLabels).
- Sellos de documento: encabezados y pies con variables (`{page}`, `{pages}`, `{date}`, `{file}`), marca de agua de texto o imagen (opacidad, rotación, delante/detrás), fondo, numeración Bates con prefijo/sufijo y dígitos, aplicados a un rango de páginas.
- Optimizar recursos compartidos al combinar (no duplicar fuentes idénticas si MuPDF lo permite).

## Interfaces que debes exponer
- `pliego_organize::{merge(inputs, out, MergeOptions, ctx), split(input, SplitRule, out_dir, ctx), extract, insert, delete, rotate, reorder(new_order), crop, set_page_labels}`; las de un solo documento sobre `&mut PdfDocument`.
- `pliego_stamp::{header_footer(HeaderFooter), watermark(Watermark), background, bates(BatesOptions)}` con opciones serializables.
- CLI: `pliego merge`, `pliego split`, `pliego pages extract|insert|delete|rotate|move|crop|labels`, `pliego stamp header|watermark|bates`.

## Criterios de "hecho"
- Tests de cada operación verificando número de páginas, orden (por texto extraído), rotación y que los marcadores apuntan a la página correcta tras combinar/reordenar.
- Combinar 50 PDFs de 20 páginas en < 5 s en acr12.
- Bates y marcas de agua visibles y legibles en Acrobat Reader (captura en el PR).
