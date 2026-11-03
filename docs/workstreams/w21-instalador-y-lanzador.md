# W21 · Instalador Pliego Setup y lanzador

Ola 3 (diseño de la app). Depende de: W01 (la app compila a `pliego-app.exe` en acr12). Puedes empezar antes con un `pliego-app.exe` de prueba.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w21` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM, WiX), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo). Si necesitas cambiar una ruta ajena, pídelo a su dueño o al hilo de
   gestión; si el diseño no cubre un caso, pregunta al hilo de diseño y, mientras, sigue con la opción más coherente con
   `docs/design/` y dilo en el PR.
2. Rama `w21-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas salvo código generado), cada uno útil por
   sí solo. Para actualizar la rama, merge de `main` (sin force-push); se fusiona con squash. Regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace` y, en `app/`, `npm run lint`, `npm run typecheck` y `npm test`. Dependencias nuevas solo con
   licencias de `deny.toml` (Rust) o MIT/ISC/Apache-2.0/BSD (npm).
4. Al abrir cada PR, envía el enlace al hilo de gestión y al de diseño. Las capturas de pantalla (claro y oscuro) van en
   el PR. Sigue la CI y las revisiones hasta que se fusione.
5. Responde a Ander en español.

## Tus rutas
`crates/pliego-install` (nuevo: formato `.paq` y disposición en disco), `crates/pliego-launcher` (nuevo: `Pliego.exe`), `crates/pliego-setup` (nuevo: instalador y desinstalador), `installer/setup/` (interfaz web del instalador y script de empaquetado). W10 conserva `installer/` para MSI y ZIP portátil, `crates/pliego-shell` y `.github/`.

## Objetivo
Un instalador `Pliego-Setup-<versión>-x86_64.exe` bonito, pequeño (< 40 MB) y fiable, más el lanzador que permite
actualizar sin reinstalar, exactamente como describe `docs/design/instalador.md` (léelo entero) y las pantallas de la
sección "Instalador" del prototipo.

## Alcance
- `crates/pliego-install` (biblioteca, sin interfaz; la usan setup, lanzador y W22):
  - Formato `.paq`: tar + zstd (crates `tar` y `zstd`, licencias permitidas) con `files.json` (ruta, tamaño, SHA-256,
    versión). `pack(dir) -> .paq`, `unpack_verified(paq, dest)` que extrae a `dest.partial`, verifica cada archivo y
    renombra. Rechaza rutas absolutas, `..` y enlaces.
  - Disposición: `current`, `pending`, `app-<v>\`, `healthy`; `stage(paq)`, `activate_pending()` atómico,
    `rollback()`, `cleanup(keep = 2)`, regla de salud (dos arranques sin `healthy` → volver atrás).
  - Pie del ejecutable (`PLIEGOPAQ1`, desplazamiento, longitud, SHA-256) para pegar el `.paq` al setup.
  - Pruebas unitarias de todo lo anterior en Linux y Windows.
- `crates/pliego-launcher` → `Pliego.exe` (y `pliego.exe` para la CLI, mismo binario según su nombre): < 500 KB,
  sin consola (`windows_subsystem = "windows"` para el de la app), activa `pending`, ejecuta la versión, aplica la regla
  de salud, fija `AppUserModelID` `Pliego.App`, pasa argumentos y código de salida. Tiempo añadido al arranque < 30 ms.
- `crates/pliego-setup` → instalador y `Uninstall.exe`:
  - Interfaz con `wry` + `tao` (MIT/Apache) cargando `installer/setup/ui/` embebido (HTML/CSS/JS sin framework o con
    SolidJS compilado, usando `tokens.css` del diseño). Ventana 680 × 440 sin marco, Mica, arrastrable, claro/oscuro,
    es/en. Pantallas: Bienvenida, Opciones, Instalando (con la animación de la hoja que se dobla), Listo, Ya instalado
    (reparar/actualizar/más reciente), Pliego está abierto, Desinstalar, Errores.
  - Sin WebView2: `TaskDialog` nativo y descarga del Evergreen Bootstrapper oficial (§5).
  - Registro en Windows de §6 (HKCU por defecto; HKLM con `--per-machine` elevado), accesos directos con
    `AppUserModelID`, PATH opcional, llamada al registro de `pliego-shell` (pídelo a W10: necesitas una función o
    ejecutable `pliego-shell register|unregister`), `SHChangeNotify`, enlace a Aplicaciones predeterminadas.
  - Línea de órdenes de §7 (silencioso, opciones, `--post-update`, `--update --paq` para instalaciones por máquina,
    `--uninstall`, `--cleanup-from`, `--pack`) y códigos de salida.
  - Registro en `%TEMP%\pliego-setup.log`.
  - El código Windows va en `platform/windows.rs` con `#![allow(unsafe_code)]` y `// SAFETY:` en cada bloque
    (crate `windows` de Microsoft, MIT/Apache).
- `installer/setup/build.ps1`: compila app + CLI + worker en release, genera `.paq` y `Pliego-Setup-*.exe`; paso de
  firma Authenticode opcional (`SIGN_TOOL`/`SIGN_ARGS`; vacío = sin firmar).
- Coordina con W10 para que la CI de publicación llame a este script (W10 es dueño de `.github/`).

## Criterios de "hecho"
- En acr12: instalar (un clic y con opciones), abrir con doble clic en un PDF tras elegir Pliego como predeterminado,
  reparar, desinstalar limpio (sin restos salvo ajustes conservados), silencioso con códigos de salida correctos,
  por usuario y por máquina. Capturas de cada pantalla en el PR, en claro y oscuro.
- Instalador < 40 MB; instalación < 10 s en acr12.
- `stage` + `activate_pending` + regla de salud probados con un `pliego-app.exe` falso que falla al arrancar.
- Accesible con teclado y Narrador; correcto a 100 %, 150 % y 200 % de escala.
