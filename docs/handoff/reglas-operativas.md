# Reglas operativas

Cómo se ha trabajado hasta el 2026-11-04 y cómo debe seguir el proyecto nuevo.

## Organización de hilos y modelos

- **Coordinador** del proyecto: habla con Ander, abre hilos, pasa mensajes.
- **Hilo de gestión** («Diseñar alternativa open source a Acrobat Pro»), con **Opus 5.5** (`claude-opus-5-5`):
  revisa y fusiona PRs, mantiene interfaces compartidas (`pliego-core`, `pliego-engine-mupdf`, `tools/acr12-*`,
  CI), reparte turnos de acr12, decide prioridades y resuelve dudas de los flujos.
- **Hilo de diseño de la app** («Diseñar app de escritorio de Pliego»), con **Opus 5.5**: dueño de `docs/design/`;
  cualquier cambio de UI, instalador o actualizaciones pasa por él (o por gestión si ya no existe).
- **Un hilo de implementación por flujo** (W01-W22), con **Sonnet 5.5** (`claude-sonnet-5-5`). Su prompt de
  arranque es `docs/workstreams/wNN-*.md` más el archivo `docs/handoff/flujos/wNN.md` con el estado actual.
- Petición original de Ander (2026-11-02): «Sonnet 5.5 para la parte agéntica y Opus 5.5 para la gestión».
- Límite práctico: unos 40 mensajes por hora entre hilos. Si se agota, los comentarios en los PRs sirven de canal.

## Autonomía

- Ander quiere **autonomía** (2026-11-03): los hilos resuelven lo pendiente sin consultarle, y solo le piden lo que
  de verdad necesita su mano: aprobar un diálogo en acr12, cerrar procesos ajenos, permisos, cuentas, dinero.
- Sí se le consulta antes: cambios de **diseño, licencias o alcance**, y cualquier publicación fuera del repo
  (issues en otros proyectos, publicar artefactos, etc.).
- No implementar a gran escala algo que cambie el diseño aprobado sin que Ander lo confirme.
- Mensajes a Ander: cortos, en español, empezando por lo que tiene que hacer.

## PRs, ramas y fusión

- Rama por PR desde `main` actualizado; PRs pequeños (< 800 líneas salvo generado), título en español,
  descripción con qué hace, cómo se probó (acr12 incluido) y qué falta. Al abrirlo, avisar a gestión con el enlace.
- **Gestión fusiona** con CI verde (Linux, Windows y frontend) tras revisar. Solo consulta a Ander si toca diseño,
  licencias o alcance.
- **Squash** siempre. **Nunca rebase ni force-push** (está bloqueado). Para actualizar una rama: `git merge
  origin/main`.
- Tras un squash, una rama apilada da conflictos add/add: tomar la versión de `main` y reaplicar el parche.
- Reutilizar una rama tras fusionar su PR, sin force-push (receta de W15): cherry-pick de los commits nuevos sobre
  `origin/main` en una rama temporal; en la rama asignada `git merge -s ours origin/main`; `git read-tree -u --reset
  <tmp>`; `git commit --amend`. El push queda fast-forward.
- `Cargo.lock` en conflicto: tomar el de `main` y dejar que cargo lo regenere; nunca resolverlo a mano.
- Licencias: solo las de `deny.toml`; una nueva necesita aprobación de gestión en el PR (y de Ander si cambia la
  licencia del proyecto).
- Propiedad de rutas y ranuras de la CLI: ver `docs/CONVENTIONS.md` y la tabla de `docs/workstreams/README.md`.

## Comprobaciones antes de cada push

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check licenses   # si hay cargo-deny
```

En Linux, `pliego-app` y `pliego-app-service` piden GTK: excluirlos (`--exclude pliego-app --exclude
pliego-app-service`) o limitarse a `-p <crate>`. `pliego-cli` necesita `libtesseract-dev libleptonica-dev` (apt). La
CI usa un clippy más nuevo que el local: evitar lints conocidos (`nonminimal_bool`, `chunks_exact` con tamaño fijo).

## Equipo de pruebas acr12

- `DESKTOP-41TJN0V`, usuario `acr12`, Windows 10 Pro 19045 (sin Mica), **16 GB de RAM**, C: pequeño (~40 GB
  libres tras la limpieza), D: grande (~500 GB). Tiene Acrobat Reader, Edge y Microsoft Office.
- Herramientas instaladas con `tools/setup-acr12.ps1` (idempotente): Git, Node LTS, LLVM 23 y **LLVM 19 en
  `D:\LLVM19`**, VS Build Tools C++, Rust, WiX 3.14, cargo-deny. Tesseract y Leptonica de vcpkg en
  `C:\Users\acr12\vcpkg`. Detalle en `docs/qa/acr12.md`.
- Acceso desde Claude: sesiones **Remote Control** en acr12 y carpetas aprobadas `C:\Users\acr12\AcrobatFree`
  (por defecto) y `C:\Users\acr12\Animacion` (esta última **no es de Pliego**: no tocar).
- Ander autorizó ejecutar PowerShell, npm, npx y cargo e instalar herramientas de desarrollo. Las reglas
  `PowerShell(npm:*)`, `PowerShell(npx:*)`, `PowerShell(cargo:*)` están en `C:\Users\acr12\.claude\settings.json`;
  solo se leen al **arrancar** una sesión, así que si cargo sale bloqueado hay que abrir una sesión nueva.
- Controlar aplicaciones con la pantalla (Acrobat Reader, Edge, Narrador, la app) necesita que **Ander acepte el
  diálogo de acceso** en el PC. Sin eso, se le pide que haga la comprobación a mano.

### Reglas de compilación en acr12 (obligatorias)

1. **Todo en D:.** Clones nuevos en `D:\dev\pliego-<flujo>` (algunos antiguos siguen en
   `C:\Users\acr12\dev\pliego-<flujo>`). Compilar e instalar en D:, nunca en C:.
2. **Cada flujo, su propio clon.** Nunca tocar ni borrar carpetas de otros flujos ni de otros proyectos de acr12.
   Solo se pueden borrar `target` y `node_modules` regenerables de clones `pliego-*`.
3. **En cada terminal, antes de compilar:** `. .\tools\acr12-env.ps1` (con punto). Pone LLVM 19 y `clang.exe` en el
   PATH (sin él, mupdf-sys falla con `max_align_t`), `CARGO_TARGET_DIR=D:\dev-cache\target\<clon>`, `TEMP`/`TMP`,
   caché de npm, navegadores de Playwright y caché de vcpkg en D:, las variables de Tesseract y
   `CARGO_BUILD_JOBS=4`. `pliego.exe` solo arranca tras cargarlo (necesita la DLL de Tesseract en el PATH).
4. **Un solo turno.** Todo lo pesado (cargo build/test/clippy, `npm run tauri build`, e2e, benchmarks) se lanza con
   `.\tools\acr12-turno.ps1 -Flujo wNN -- <orden>`. Usa el mutex `Global\PliegoAcr12TurnoUnico`, espera por orden
   de llegada (FIFO), escribe quién lo tiene y su PID en `D:\dev-cache\turnos\turno.txt`, sale con 75 si se agota
   `-EsperaMaxMinutos` (240 por defecto) y si no con el código de la orden. Antes de usarlo, `git merge
   origin/main` en el clon para tener la versión buena.
5. **Lo que no termina no va por turno.** `tauri dev` y otros servidores retienen el turno: compilar por turno
   (`cargo build -p pliego-app` o `npm run tauri build`) y ejecutar el exe fuera del turno.
6. **Cerrar lo propio al acabar.** Los hilos no pueden cerrar procesos de otros; si hay compilaciones colgadas
   ajenas, se le pide a Ander (ejemplo usado:
   `Get-Process cargo,rustc,link -EA 0 | ? StartTime -lt (Get-Date '21:30') | Stop-Process -Force`).
7. **Solo lo que no se puede probar en Linux.** Fuzzing largo y benchmarks de Linux van en la CI nocturna, no en
   acr12. Benchmarks de acr12 solo con la máquina tranquila y el turno cogido.
8. Clones creados antes de `tests/corpus/.gitattributes` pueden tener los PDF con CRLF (xref rota, «object ID 4 0
   not found»): `git checkout -- tests/corpus` o reclonar.
9. Gestión reparte el turno cuando hay cola y avisa a los flujos de cada cambio en los scripts.
