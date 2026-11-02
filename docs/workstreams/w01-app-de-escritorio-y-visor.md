# W01 · App de escritorio y visor

Ola 1. Depende de: base existente (pliego-core, pliego-engine-mupdf).

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w01` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w01-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Rebase sobre `main` antes de abrir el PR; regenera `Cargo.lock`, no lo resuelvas a mano.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`app/` (frontend SolidJS en `app/src`, backend Tauri en `app/src-tauri`, que se añade como miembro del workspace).

## Objetivo
Crear la aplicación de escritorio de Pliego con identidad visual propia y un visor rápido por teselas. Es la base donde los demás flujos integrarán sus funciones.

## Alcance
- Proyecto Tauri 2 (WebView2) + SolidJS + TypeScript estricto + Vite. Añadir `app/src-tauri` a `members` del workspace raíz (único cambio permitido en ese archivo).
- Visor: desplazamiento continuo y página a página, zoom (ajustar ancho/página, 10 %–1600 %), rotación de vista, teselas de 512 px servidas por protocolo `pliego://tile/...` desde un hilo por documento usando `pliego-engine-mupdf`; render progresivo (baja resolución primero).
- Pestañas con varios documentos, panel lateral con miniaturas, marcadores (outline), búsqueda con resaltado y navegación entre coincidencias, selección y copia de texto, documentos recientes, abrir con contraseña.
- Arrastrar y soltar archivos, abrir desde línea de comandos (`pliego-app.exe archivo.pdf`) y una sola instancia.
- Sistema de diseño propio: `app/src/styles/tokens.css` (colores, tipografía, espaciado), tema claro/oscuro siguiendo Windows, iconos propios (SVG libres, p. ej. Lucide ISC). Bocetos en `docs/design/` antes de la interfaz final: avisa al hilo de gestión para que Ander los apruebe.
- Interfaz en español e inglés (i18n desde el principio, archivos `app/src/i18n/es.json`, `en.json`).
- Registro de comandos: `app/src-tauri/src/commands/` con un archivo por área, para que otros flujos añadan los suyos sin conflictos.

## Interfaces que debes exponer
- Comandos Tauri: `open_document(path, password?) -> DocId`, `close_document`, `document_info`, `page_sizes`, `search`, `outline`, `page_text`.
- Eventos: `document-changed {doc_id, revision}` para invalidar teselas cuando otro módulo modifique el documento.
- Guía corta `docs/app-integration.md`: cómo un módulo añade un comando Tauri, una entrada de menú/barra y un panel.

## Criterios de "hecho"
- Abre y navega con fluidez un PDF de 500 páginas en acr12 (abrir < 1,5 s, desplazamiento sin saltos visibles a 60 Hz en zoom 100 %).
- Memoria en reposo < 150 MB con un documento de 100 páginas abierto.
- Tests: Rust para el servidor de teselas y comandos; Playwright o pruebas de componentes para la interfaz básica.
- `npm run tauri build` genera un ejecutable en acr12; captura de pantalla adjunta al PR.
- Bocetos aprobados por Ander (vía hilo de gestión) antes de fusionar el diseño final.
