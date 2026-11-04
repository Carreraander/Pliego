# Memoria del proyecto para reconstruir

El proyecto nuevo empieza con la memoria vacía. El coordinador debe crear estos archivos de memoria (uno por tema),
copiando o resumiendo el texto. Todo lo demás se puede consultar en el repo y no hace falta duplicarlo.

## proyecto-pliego (project)
Alternativa libre (AGPL-3.0 + MuPDF, decidido por Ander el 2026-11-02) y ligera a Acrobat Pro para Windows.
Repo `github.com/Carreraander/Pliego`. Hablar en español. Estado y decisiones: `docs/handoff/README.md`. Organización:
hilo de gestión con Opus 5.5, un hilo Sonnet 5.5 por flujo (W01-W22), hilo de diseño de la app con Opus 5.5. Prompts
de flujo en `docs/workstreams/`, estado de cada uno en `docs/handoff/flujos/`. Traslado de cuenta el 2026-11-04.

## ander-preferencias (user / feedback)
- Ander quiere autonomía: resolver sin consultarle salvo permisos reales (aprobar diálogos en acr12, cerrar procesos
  ajenos). Sí consultarle diseño, licencias, alcance y publicaciones fuera del repo.
- Mensajes cortos en español, empezando por lo que tiene que hacer.
- Quiere poder comercializarlo algún día; sabe que con AGPL + MuPDF una edición cerrada exige licencia de Artifex.
- No usar marca ni interfaz de Adobe.

## reglas-fusion-y-ramas (feedback)
Gestión fusiona con CI verde tras revisar; squash; actualizar con merge de main; nunca rebase ni force-push (bloqueado).
Consultar a Ander solo PRs que toquen diseño, licencias o alcance. Detalle: `docs/handoff/reglas-operativas.md`.

## acr12 (reference / project)
`DESKTOP-41TJN0V`, usuario `acr12`, Windows 10 (sin Mica), 16 GB RAM, C: pequeño, D: grande. Compilar e instalar en D:
(clones en `D:\dev\pliego-<flujo>`), `. .\tools\acr12-env.ps1` en cada terminal, todo lo pesado con
`.\tools\acr12-turno.ps1 -Flujo wNN -- <orden>` (un solo turno, FIFO, `D:\dev-cache\turnos\turno.txt`),
`CARGO_BUILD_JOBS=4`, `tauri dev` fuera de turno. Los hilos no cierran procesos ajenos: lo hace Ander. npm/npx/cargo
permitidos en `C:\Users\acr12\.claude\settings.json` (se leen al arrancar la sesión). Carpetas aprobadas:
`C:\Users\acr12\AcrobatFree` (por defecto) y `C:\Users\acr12\Animacion` (no es de Pliego). Detalle:
`docs/handoff/reglas-operativas.md` y `docs/qa/acr12.md`.

## pliego-diseno-app (project)
Diseño de app, instalador y actualizaciones en `docs/design/` (PR #28, #82). Modos fijos + Herramientas + Ctrl+K,
estilo «grafito y tinta», instalador propio por usuario, actualizaciones Ed25519 en VPS de Ander (pendiente). La clave
secreta de producción la genera Ander sin conexión; nunca al repo ni al VPS.

## pendientes-ander (project)
Lista viva en `docs/handoff/README.md` § «Pendiente de Ander»: VPS, PDFs de W11 en Reader, permiso de pantalla para
W20/W03, criterio de memoria de W01, lector DNIe, modelos GGUF en C:, exclusión del antivirus.

## hilos (reference)
Tabla con el id del hilo de gestión, el de diseño y el de cada flujo del proyecto nuevo (rellenar al abrirlos).
