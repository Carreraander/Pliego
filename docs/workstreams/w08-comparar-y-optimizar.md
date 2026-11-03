# W08 · Comparar y optimizar

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w08` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w08-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Para actualizar la rama, haz merge de `main` en ella (nunca rebase ni force-push); regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-compare`, `crates/pliego-optimize`, `crates/pliego-cli/src/commands/{compare,optimize}.rs`.

## Objetivo
Comparar dos versiones de un documento con informe claro, y reducir el tamaño de los PDFs con preajustes.

## Alcance
- Comparación de texto: alineación de páginas (insertadas/borradas), diff por palabras con posición en página, cambios de formato básicos (fuente/tamaño) cuando estén disponibles.
- Comparación visual: diferencia por píxeles de páginas emparejadas con umbral y agrupación en regiones.
- Informe: PDF con ambas versiones resaltadas (añadido verde, borrado rojo, cambiado ámbar) y página de resumen; también JSON.
- Optimizar: recompresión y submuestreo de imágenes (JPEG con calidad configurable; mozjpeg si la licencia y el empaquetado lo permiten; JBIG2/CCITT para monocromo si es viable), subconjuntos de fuentes, quitar objetos duplicados y sin referencia, comprimir flujos, linealizar para web.
- Preajustes: `pantalla` (96 ppp), `ebook` (150 ppp), `impresion` (300 ppp), `sin-perdida`. Informe de tamaño antes/después por categoría.

## Interfaces que debes exponer
- `pliego_compare::{compare(a, b, CompareOptions, ctx) -> Comparison, write_report(&Comparison, out)}`.
- `pliego_optimize::{optimize(input, out, OptimizeOptions, ctx) -> Report, analyze(input) -> SpaceUsage}`.
- CLI: `pliego compare a.pdf b.pdf -o informe.pdf [--json]`, `pliego optimize entrada.pdf -o salida.pdf --preset ebook`, `pliego optimize --analyze`.

## Criterios de "hecho"
- Tests con pares de PDFs generados: palabra cambiada, párrafo insertado, página insertada, imagen movida; todos detectados en la página correcta.
- `optimize --preset ebook` reduce al menos un 50 % un PDF de escaneos a 300 ppp sin perder la capa de texto; `sin-perdida` nunca cambia el render (comparación por píxel).
- El PDF optimizado abre sin avisos en Acrobat Reader.
