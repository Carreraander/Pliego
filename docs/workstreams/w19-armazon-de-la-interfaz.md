# W19 · Armazón de la interfaz y sistema de diseño

Ola 3 (diseño de la app). Depende de: W01 con el andamiaje Tauri + SolidJS en `main` (si aún no está, empieza por los componentes en una página de pruebas de Vite y adáptate cuando llegue).

## Contexto (léelo entero)
Eres una sesión de implementación del proyecto **Pliego**: una alternativa libre (AGPL-3.0-or-later) y gratuita a
Adobe Acrobat Pro para Windows. Ander es el dueño del proyecto. Hay un hilo de gestión ("Diseñar alternativa open
source a Acrobat Pro", id `cmsg_01VzNAbDDgN8F4HKcyc76pSD2RqHEmBxL1s12gPvzmcf2G`) que coordina todos los flujos, revisa
PRs y decide sobre interfaces compartidas, y un hilo de diseño ("Diseñar app de escritorio de Pliego", id
`cmsg_01VzNAbDDgN8F4HKcyc76pSDAmAF7yZBZEz28eJnRWPjoG`) dueño del diseño de la app, el instalador y las actualizaciones.
Escríbeles con `message_thread` usando esos ids: dudas de diseño o UX al hilo de diseño; PRs, rutas e interfaces
compartidas al de gestión.

- Repositorio: https://github.com/Carreraander/Pliego (rama `main`). Añádelo a tu sesión y clónalo.
- **Diseño que implementas (léelo entero antes de escribir código):** `docs/design/` —
  `README.md`, `ux-y-navegacion.md`, `sistema-visual.md`, `tokens.css`, `instalador.md`, `actualizaciones.md` y el
  prototipo navegable `prototipo.html` (también en https://claude.ai/artifact/Pin9i2YGZRQUuDdft48FVR). El prototipo es
  la referencia visual: cópialo en espíritu y medidas, no su código (es HTML de demostración).
- Lee también: `CLAUDE.md`, `docs/CONVENTIONS.md`, `docs/ARCHITECTURE.md`.
- Stack: Rust (edición 2024) + MuPDF vía crate `mupdf` 0.8 (AGPL) · Tauri 2 + WebView2 + SolidJS + TypeScript estricto
  + Vite · CLI `pliego`.
- Equipo de pruebas Windows: usuario `acr12` en `DESKTOP-41TJN0V`. Para compilar o probar allí, pide una sesión
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w19` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM, WiX), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo). Si necesitas cambiar una ruta ajena, pídelo a su dueño o al hilo de
   gestión; si el diseño no cubre un caso, pregunta al hilo de diseño y, mientras, sigue con la opción más coherente con
   `docs/design/` y dilo en el PR.
2. Rama `w19-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas salvo código generado), cada uno útil por
   sí solo. Para actualizar la rama, merge de `main` (sin force-push); se fusiona con squash. Regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace` y, en `app/`, `npm run lint`, `npm run typecheck` y `npm test`. Dependencias nuevas solo con
   licencias de `deny.toml` (Rust) o MIT/ISC/Apache-2.0/BSD (npm).
4. Al abrir cada PR, envía el enlace al hilo de gestión y al de diseño. Las capturas de pantalla (claro y oscuro) van en
   el PR. Sigue la CI y las revisiones hasta que se fusione.
5. Responde a Ander en español.

## Tus rutas
`app/src/styles/` (incluido `tokens.css`), `app/src/ui/` (componentes e iconos), `app/src/shell/` (ventana, barras, paneles, inspector), `app/tests/visual/`. W01 conserva `app/src/viewer`, `app/src/state`, `app/src/api`, `app/src/panels` y `app/src-tauri`.

## Objetivo
Construir el armazón visual de la app tal como lo define `docs/design/`: tokens, componentes reutilizables y la
ventana completa (barra de título, barra de modos, raíl, panel lateral, lienzo, inspector y barra de estado), para que
el visor de W01 y las herramientas de los demás flujos se monten dentro sin crear estilos propios.

## Alcance
- `app/src/styles/tokens.css` = copia exacta de `docs/design/tokens.css`; `base.css` (reset, tipografía, foco visible,
  `forced-colors`, `prefers-reduced-motion`). Tema: sigue Windows; Ajustes puede forzar claro/oscuro con `data-theme`;
  densidad con `data-density`.
- Componentes en `app/src/ui/` (SolidJS, accesibles, con estados hover/pulsado/foco/desactivado): `Button` (primario,
  normal, fantasma, peligro), `IconButton`, `ToggleGroup`, `Menu` y `ContextMenu`, `Tooltip` (con atajo), `Field`,
  `TextInput`, `Select`, `Slider`, `Checkbox`, `Radio`, `Switch`, `Tabs`, `Kbd`, `Chip`, `Notice` (barra de aviso),
  `Toast`, `EmptyState`, `Inspector`, `TaskPanel` (título, opciones, avanzado plegado, estimación, pie con acciones y
  progreso), `WizardSteps`, `Popover`. Iconos: Lucide (ISC) vía `lucide-solid` o SVG importados, más los propios de
  `app/src/ui/icons/custom/` (redactar, Bates, PDF/A, OCR, sello de tiempo, logo).
- `app/src/shell/`: ventana sin marco con barra de título propia (pestañas arrastrables y reordenables, menú de la app,
  campo de búsqueda que abre la paleta de órdenes —la paleta la hace W20—, hueco para la pastilla de actualización,
  botones de ventana con Snap Layouts: evalúa `tauri-plugin-decorum` u otra solución con licencia permitida),
  Mica en Windows 11 (`windowEffects`), barra de modos con hueco para modos y barra de herramientas del modo, raíl de
  paneles con registro de paneles, panel lateral redimensionable (200–480 px, recuerda ancho), inspector (280–480 px),
  barra de estado con ranuras, comportamiento < 1100 px y < 900 px (§2 de `ux-y-navegacion.md`), regiones con `F6`.
- Logo: SVG del prototipo y generación de `app/src-tauri/icons/*` y `.ico` (16–256 px) con un script en
  `app/scripts/` (pide a W01 que acepte los iconos en `src-tauri/icons`).
- Página de catálogo de componentes (`/ui-catalog` solo en desarrollo) para revisar todos los estados.
- Pruebas: Playwright con capturas de referencia del catálogo y del armazón en claro, oscuro y alto contraste;
  pruebas de accesibilidad con `@axe-core/playwright` (0 infracciones graves).

## Interfaces que debes exponer
- `app/src/shell/slots.ts`: API para que otros registren pestañas de paneles del raíl, contenido del inspector,
  elementos de la barra de estado y avisos del documento (`registerPanel`, `openInspector(component, props)`,
  `showNotice(docId, notice)`, `registerStatusItem`).
- Documenta en `docs/app-integration.md` (sección "Interfaz") cómo usar componentes y ranuras. Coordina el archivo con
  W01 y W20.

## Criterios de "hecho"
- El armazón con el visor de W01 dentro se ve como el prototipo en claro y oscuro (capturas en el PR) y respeta
  medidas y tokens.
- Todo el armazón se usa solo con teclado; Narrador lee pestañas, modos, botones y paneles (comprobado en acr12).
- Arranque en frío de la app < 1 s en acr12 sigue cumpliéndose (mide antes y después).
- Ningún componente usa colores literales (comprobación con un test que busque `#` y `rgb(` fuera de `tokens.css`).
