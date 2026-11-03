# W11 · Edición de texto e imágenes

Ola 2. Depende de: W02 (pliego-document) fusionado.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w11` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w11-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Para actualizar la rama, haz merge de `main` en ella (nunca rebase ni force-push); regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-edit`, `crates/pliego-cli/src/commands/edit.rs`.

## Objetivo
Editar el contenido de páginas existentes: texto con reflujo de párrafo, imágenes y objetos, que es la función más difícil del producto.

## Alcance
- Detección de bloques de texto editables (párrafos) a partir del texto estructurado y de los operadores del flujo de contenido.
- Editar texto: reemplazar en línea, insertar y borrar con reflujo dentro del bloque; fuentes: reutilizar la incrustada si tiene los glifos, si no, fuente sustituta parecida (con aviso), con HarfBuzz para el shaping.
- Añadir texto nuevo, cambiar fuente/tamaño/color.
- Imágenes: insertar, reemplazar, mover, redimensionar, rotar, recortar, borrar. Objetos vectoriales: mover y borrar.
- Enlaces: crear y editar enlaces internos y externos.

## Interfaces que debes exponer
- `pliego_edit::{text_blocks(page) -> Vec<TextBlock>, replace_text(block, new_text, Style) -> Report, insert_text, insert_image, replace_image, transform_object, delete_object, add_link}` sobre `pliego_document::Document`.

## Criterios de "hecho"
- Corpus de 30 PDFs variados (Word, LaTeX, escaneos con OCR, CJK): editar una palabra deja el resto del párrafo idéntico visualmente (comparación por píxeles fuera del bloque).
- Las ediciones se guardan y abren bien en Acrobat Reader.
- Avisos claros cuando una fuente no permite editar.
