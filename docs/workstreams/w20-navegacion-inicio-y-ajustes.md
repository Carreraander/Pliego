# W20 · Navegación de herramientas, Inicio y Ajustes

Ola 3 (diseño de la app). Depende de: W19 (armazón y componentes) en `main`.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w20` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM, WiX), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo). Si necesitas cambiar una ruta ajena, pídelo a su dueño o al hilo de
   gestión; si el diseño no cubre un caso, pregunta al hilo de diseño y, mientras, sigue con la opción más coherente con
   `docs/design/` y dilo en el PR.
2. Rama `w20-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas salvo código generado), cada uno útil por
   sí solo. Para actualizar la rama, merge de `main` (sin force-push); se fusiona con squash. Regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace` y, en `app/`, `npm run lint`, `npm run typecheck` y `npm test`. Dependencias nuevas solo con
   licencias de `deny.toml` (Rust) o MIT/ISC/Apache-2.0/BSD (npm).
4. Al abrir cada PR, envía el enlace al hilo de gestión y al de diseño. Las capturas de pantalla (claro y oscuro) van en
   el PR. Sigue la CI y las revisiones hasta que se fusione.
5. Responde a Ander en español.

## Tus rutas
`app/src/tools/` (registro y manifiestos de las herramientas base), `app/src/navigation/` (barra de modos, paleta de herramientas, paleta de órdenes, atajos), `app/src/home/`, `app/src/settings/`, `app/src/jobs/` (centro de tareas), `app/src-tauri/src/commands/{settings,jobs,recents}.rs`.

## Objetivo
Hacer que todas las funciones de Pliego se encuentren y se usen igual: registro de herramientas, barra de modos, paleta
"Herramientas", paleta de órdenes `Ctrl+K`, atajos, pantalla de Inicio, Ajustes y centro de tareas, según
`docs/design/ux-y-navegacion.md` §3–§6 y §10.

## Alcance
- `app/src/tools/registry.ts`: tipo `ToolManifest` exactamente como en §10 de `ux-y-navegacion.md` (si necesitas
  cambiarlo, acuérdalo con el hilo de diseño) y registro con carga perezosa. Condiciones `when` (documento abierto, tiene
  formularios, tiene firmas, está protegido, es PDF/A…) evaluadas contra el estado del documento de W01.
- Barra de modos: seis modos por defecto, "Herramientas ▾", fijar/soltar/reordenar (persistido en ajustes),
  `Alt+1…9`, plegado a desplegable en ventanas estrechas. Barra de herramientas del modo generada desde el manifiesto
  del modo, con atajos de una letra cuando el foco está en el lienzo.
- Paleta "Herramientas" (`Ctrl+Mayús+T`): grupos de §4.3, buscador, herramientas desactivadas con motivo.
- Paleta de órdenes (`Ctrl+K`): proveedores enchufables (órdenes, documentos abiertos, recientes, "ir a página N",
  buscar texto en el documento, ajustes); búsqueda aproximada sin tildes ni mayúsculas; atajo visible; navegación con
  flechas, `Enter`, `Tab`, `Esc`.
- Atajos: tabla de §6 con un gestor central (sin conflictos, configurable) y hoja de atajos con `F1`.
- Inicio: novedades (tras actualizar), saludo, zona de soltar, acciones rápidas personalizables, recientes con
  miniatura, carpeta, fecha y estado; fijar; archivo desaparecido; recuperación tras cierre inesperado (con W02).
- Ajustes como pestaña: secciones de §3.5 con buscador; implementa General, Visor, Accesibilidad, Avanzado (atajos) y
  Acerca de (licencia AGPL, versiones de MuPDF y componentes, enlace al código fuente, obligatorio por AGPL). Deja la
  sección Actualizaciones con la interfaz del prototipo conectada a un servicio simulado: W22 la conectará al real.
  Persistencia en `%APPDATA%\Pliego\settings.json` con esquema versionado.
- Centro de tareas: escucha `job-progress`/`job-finished`, popover desde la barra de estado, cancelar, abrir resultado,
  mostrar en carpeta, notificación de Windows si la ventana no está en primer plano.
- Manifiestos de las herramientas que ya existen en `main` (las de W01: Leer, Presentación, Capas, Adjuntos,
  Propiedades) y "fantasmas" desactivados para el resto, para que la paleta muestre ya el mapa completo.
- `docs/app-integration.md` (sección "Herramientas"): cómo un flujo añade su `ToolManifest`, su comando Tauri y su
  panel de tarea, con un ejemplo completo (p. ej. Comprimir llamando a `pliego-optimize`).

## Criterios de "hecho"
- Todas las herramientas de la tabla §4.3 aparecen en la paleta y en `Ctrl+K` (activas o desactivadas con motivo).
- Un flujo puede añadir una herramienta tocando solo su carpeta `app/src/tools/<area>/` y una línea de registro
  (demuéstralo en el PR con Comprimir o Rotar páginas).
- Pruebas de componentes y Playwright de: cambio de modo, paleta, `Ctrl+K`, atajos, Inicio y Ajustes; capturas en claro
  y oscuro en el PR; probado con teclado y Narrador en acr12.
