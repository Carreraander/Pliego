# W22 · Autoactualizaciones centralizadas

Ola 3 (diseño de la app). Depende de: W21 (`pliego-install`) y W20 (Ajustes) en `main` para la integración; el servidor y el cliente pueden empezar ya.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w22` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM, WiX), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo). Si necesitas cambiar una ruta ajena, pídelo a su dueño o al hilo de
   gestión; si el diseño no cubre un caso, pregunta al hilo de diseño y, mientras, sigue con la opción más coherente con
   `docs/design/` y dilo en el PR.
2. Rama `w22-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas salvo código generado), cada uno útil por
   sí solo. Para actualizar la rama, merge de `main` (sin force-push); se fusiona con squash. Regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace` y, en `app/`, `npm run lint`, `npm run typecheck` y `npm test`. Dependencias nuevas solo con
   licencias de `deny.toml` (Rust) o MIT/ISC/Apache-2.0/BSD (npm).
4. Al abrir cada PR, envía el enlace al hilo de gestión y al de diseño. Las capturas de pantalla (claro y oscuro) van en
   el PR. Sigue la CI y las revisiones hasta que se fusione.
5. Responde a Ander en español.

## Tus rutas
`crates/pliego-update` y `crates/pliego-release` (ya existen: prototipo del hilo de diseño, ahora tuyos), `app/src/updates/` (pastilla, popover, aviso de obligatoria, conexión de Ajustes > Actualizaciones), `app/src-tauri/src/commands/updates.rs`, `deploy/updates/` (nuevo: Caddyfile, preparación del VPS, script de subida), `docs/releasing.md`.

## Objetivo
Autoactualizaciones centralizadas y seguras: el VPS de Ander (que llegará después) sirve archivos estáticos con
manifiestos firmados, y Pliego se actualiza solo sin molestar, según `docs/design/actualizaciones.md` (léelo entero).
Todo debe funcionar y probarse ya con el servidor local `pliego-release serve`.

## Punto de partida
`crates/pliego-update` (cliente: sobre firmado Ed25519, canales, caducidad, serial anti-reproducción, despliegue
gradual, obligatorias, descarga verificada, configuración con directiva/entorno/usuario/compilación; 17 pruebas) y
`crates/pliego-release` (`keygen`, `publish`, `resign`, `verify`, `serve`, `check`) ya funcionan; se probaron de
extremo a extremo en local. Léelos antes de cambiar nada y mantén sus pruebas en verde.

## Alcance
- Cliente:
  - Programación de comprobaciones (30 s tras arrancar, cada 6 h, como mucho 1 por hora), conexión de uso medido de
    Windows (solo comprobar), modo y canal desde Ajustes, directiva `HKLM\SOFTWARE\Policies\Pliego\Updates`
    (lectura en `platform/windows.rs`), estado en `%LOCALAPPDATA%\Pliego\update-state.json` con `install_id` aleatorio
    (`getrandom`).
  - Aplicar: descargar → `pliego_install::stage` → `pending`; "Reiniciar ahora" guarda (o pide guardar) y sale lanzando
    `Pliego.exe`; instalaciones por máquina con `pliego-setup --update --paq` elevado; ZIP portátil solo avisa;
    MSI sin actualizaciones (detectado por una marca en la instalación).
  - Versión omitida, vuelta atrás comunicada al usuario, tarjeta de novedades en Inicio (con W20).
- App (Tauri + SolidJS): comandos `update_status`, `update_check_now`, `update_download`, `update_restart_now`,
  `update_set_settings` y evento `update-state-changed`; pastilla "Actualización lista" en la barra de título (ranura de
  W19), popover con notas y "Reiniciar ahora"/"Al cerrar Pliego", aviso persistente para obligatorias, y la sección
  Ajustes > Actualizaciones de W20 conectada al servicio real. Todo como en el prototipo.
- Publicación:
  - `docs/releasing.md`: crear la clave (Ander, sin conexión), compilar con `PLIEGO_UPDATE_URL` y
    `PLIEGO_UPDATE_PUBKEYS`, publicar, ampliar despliegue, volver a firmar, rotar claves, retirar una versión
    (`--block`).
  - Flujo de GitHub Actions de publicación por etiqueta `v*` (propón el archivo a W10, dueño de `.github/`): compila en
    Windows, llama a `installer/setup/build.ps1` (W21), firma el manifiesto con la clave de un entorno protegido
    `release` con aprobación de Ander, crea la Release de GitHub y sube el árbol al VPS por `rsync` (desactivado hasta
    que exista el secreto del VPS). Trabajo mensual de `resign`.
  - `deploy/updates/`: `Caddyfile` del diseño, `setup-vps.sh` idempotente (usuario `pliego` sin shell para `rsync`,
    carpeta `/srv/pliego-actualizaciones`, Caddy) y `README.md` con los pasos para cuando Ander tenga el VPS y dominio.
- Deltas: fuera de alcance (fase 2), pero deja el campo `kind` preparado.

## Criterios de "hecho"
- Prueba de extremo a extremo en acr12 documentada en el PR con capturas: instalar 0.1.0 con el instalador de W21,
  `pliego-release serve` en acr12 con 0.1.1 publicada (`PLIEGO_UPDATE_URL=http://127.0.0.1:8787`), la app detecta,
  descarga y verifica, muestra la pastilla, "Reiniciar ahora" deja la 0.1.1 funcionando; un paquete manipulado se
  rechaza; una 0.1.2 que no arranca vuelve sola a 0.1.1.
- Sin URL o sin claves compiladas, la app no hace ninguna petición (prueba).
- Ajustes > Actualizaciones muestra "Gestionado por tu organización" con la directiva puesta.
