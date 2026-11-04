# Ola 4: conectar las herramientas a la app (W23-W29)

## Por qué

Las olas 1 y 2 hicieron cada función como **crate + orden de la CLI `pliego`**, con pruebas en Linux, CI y en parte en
acr12. La ola 3 hizo el armazón de la app (modos, paleta, Ctrl+K, centro de tareas, Inicio, Ajustes). Pero **ningún
flujo tenía asignado conectar su crate a la interfaz**: en `main` (2026-11-04) la app solo tiene de verdad Leer, la
búsqueda y el asistente; las otras ~45 herramientas de `app/src/tools/base.ts` están registradas como «fantasma» y salen
como «próximamente» / «Disponible más adelante», y los cinco modos que no son Leer muestran «Esta herramienta llegará en
una próxima versión».

Esta ola cierra ese hueco **sin cambiar el diseño aprobado**: implementa el contrato de `docs/design/ux-y-navegacion.md`
§10 y `docs/app-integration.md` («Herramientas» y «Tareas en segundo plano»), con el ejemplo de Comprimir como patrón.

## Reglas comunes

- Cada herramienta sustituye a su fantasma registrando un `ToolManifest` con **el mismo `id`** desde
  `app/src/tools/<area>/` (manifiesto, panel con `TaskPanel`, textos es/en con `addToolStrings`) y **una línea** en
  `app/src/tools/index.ts`. Los modos se registran con `registerMode` y el mismo id (sustituyen al marcador de
  `navigation/modes.tsx`).
- Backend: `app/src-tauri/src/commands/<area>.rs` y una línea en `commands/mod.rs` y en el `generate_handler!`. La
  dependencia del crate va en `app/src-tauri/Cargo.toml`. Las operaciones largas son **trabajos** (`JobsState::spawn`,
  progreso y cancelación con `JobContext`), visibles en el centro de tareas.
- Por defecto se **guarda una copia** (`<nombre> (comprimido).pdf`, etc., junto al original) y se ofrece «Abrir
  resultado»; si se modifica el documento abierto, `bump_revision(docId)` al terminar.
- Archivos compartidos (`commands/mod.rs`, `lib.rs`, `tools/index.ts`, `Cargo.toml` de la app): **solo añadir líneas**,
  y actualizar la rama con `git merge origin/main` antes de pedir la fusión. `Cargo.lock` en conflicto: tomar el de
  `main` y regenerar.
- Pruebas: test de cada comando en Rust (sin Tauri: separar la lógica en una función que reciba rutas), test de
  componente o Playwright del panel con `invoke` simulado, y paridad de textos es/en. Capturas claro/oscuro en el PR
  desde `/nav-demo` o la app.
- acr12: compilar la app por turno (`acr12-turno.ps1`), probar la orden de punta a punta sin control de pantalla; la
  revisión visual va a `gestion/revision-manual-ander.md` en una línea.
- PRs pequeños: un PR por grupo de herramientas (< 800 líneas). Al terminar, actualizar `docs/handoff/flujos/wNN.md`.

## Flujos y orden de prioridad

| Flujo | Tema | Herramientas (`id` de `base.ts`) | Crates | Notas |
| --- | --- | --- | --- | --- |
| **W23** | Organizar y combinar | modo `pages.organize` (miniaturas, reordenar, girar, borrar, extraer), `pages.merge`, `pages.split`, `pages.insert`, `pages.number` | `pliego-organize` | Primero: es el uso más frecuente. Su primer PR deja un ayudante común «trabajo que guarda copia y ofrece abrirla» que reutilizan los demás. |
| **W24** | Convertir, comprimir y OCR | `optimize.compress`, `optimize.advanced`, `ocr.recognize`, `ocr.clean`, `ocr.languages`, `convert.create`, `convert.export`, `convert.office`, `convert.pdfa`, `compare.compare` | `pliego-optimize`, `pliego-ocr`, `pliego-convert`, `pliego-office-export`, `pliego-pdfa`, `pliego-compare` | Todo son tareas archivo → archivo: el patrón de Comprimir tal cual. `compare.compare` es pantalla (`openScreen`). |
| **W25** | Proteger y redactar | modo `protect.protect` (contraseña, permisos, AES), modo `protect.redact` (marcar áreas y patrones de datos personales, aplicar), `protect.sanitize` | `pliego-protect`, `pliego-redact` | Abrir PDFs con contraseña ya existe (`open_document(password)`). |
| **W26** | Comentar y rellenar | modo `annotate.comment` (resaltar, subrayar, nota, dibujo a mano sobre el lienzo), `annotate.xfdf`, `annotate.summary`, modo `forms.fill` (campos sobre el visor, «Mi firma» manuscrita) | `pliego-annot`, `pliego-forms` | Necesita capa interactiva sobre el visor de W01: coordinar con `app/src/viewer/` sin romperlo. |
| **W27** | Sellos, marca de agua y páginas | modo `stamp.stamps`, `edit.watermark`, `edit.headerfooter`, `edit.background`, `edit.bates` | `pliego-stamp` | Tareas con vista previa de la página actual. |
| **W28** | Firmas | `sign.digital` (asistente: certificado del almacén de Windows, PFX o DNIe; nivel B-B/B-T/B-LT/B-LTA), `sign.validate` (panel), `sign.workflow` | `pliego-sign`, `pliego-signflow` | Fija `setDocFacts({ signatures })`. |
| **W29** | Editar, formularios y avanzado | modo `edit.content`, `forms.design`, `forms.detect`, `access.check`, `access.order`, `access.validate-pdfa`, `print.preview`, `print.preflight`, `batch.actions`, `batch.index`, `batch.measure`, `batch.portfolio`, y las de Ver de W01 (`viewer.present`, `viewer.layers`, `viewer.attachments`, `viewer.properties`) | `pliego-edit`, `pliego-formdesign`, `pliego-a11y`, `pliego-print`, `pliego-actions`, `pliego-index`, `pliego-measure`, `pliego-portfolio` | Lo menos usado o lo más complejo (edición de texto en el lienzo). Puede partirse en dos hilos. |

W23 empieza primero; W24 y W25 pueden ir en paralelo desde el principio usando el ejemplo de Comprimir; W26-W29 tras el
primer PR de W23. Cuando todas estén conectadas, gestión regenera la beta (`tools/build-beta.ps1`) y la lista de
comprobación manual de Ander se amplía con un recorrido por los seis modos.

## Criterio de hecho de la ola

- Ninguna herramienta de `base.ts` queda como fantasma (o la que quede, con decisión de Ander y motivo en este archivo).
- Los seis modos tienen barra propia; ningún «próximamente» en la beta.
- Cada herramienta tiene test de comando y de panel; CI verde en Linux, Windows y frontend.
