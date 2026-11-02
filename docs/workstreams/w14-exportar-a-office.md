# W14 · Exportar a Office

Ola 2. Depende de: W09 (`structured_text`) fusionado.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w14` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w14-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Rebase sobre `main` antes de abrir el PR; regenera `Cargo.lock`, no lo resuelvas a mano.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-office-export`, `crates/pliego-cli/src/commands/office.rs`.

## Objetivo
Exportar PDFs a Word, Excel y PowerPoint conservando la maquetación tanto como sea posible.

## Alcance
- DOCX: reconstrucción de párrafos, columnas, encabezados/pies, listas, tablas, imágenes y estilos de fuente a partir del texto estructurado (referencia de algoritmo: pdf2docx, sin copiar su código). Escritura OOXML propia o con una crate MIT/Apache.
- XLSX: detección de tablas (por líneas y alineación) a hojas, una por tabla o por página.
- PPTX: una diapositiva por página con texto editable e imágenes.
- PDFs escaneados: avisar y ofrecer OCR (W06) antes de exportar.

## Interfaces que debes exponer
- `pliego_office_export::{to_docx(input, out, DocxOptions, ctx), to_xlsx, to_pptx}`.

## Criterios de "hecho"
- Corpus de 20 PDFs (cartas, facturas, artículos a dos columnas, tablas): el DOCX abre sin errores en Word o LibreOffice y conserva el orden de lectura; revisión visual en acr12 con capturas.
- Las tablas de facturas de prueba salen como tablas reales en DOCX y XLSX.
