# Flujos de trabajo

Cada archivo `wNN-*.md` es un prompt autocontenido para una sesión de implementación. La gestión (revisión de PRs,
interfaces compartidas, prioridades) la lleva el hilo de gestión.

- **Ola 1** (en paralelo desde ya): W01 a W10.
- **Ola 2**: W11 a W18, cada uno cuando su dependencia esté fusionada en `main`.
- **Ola 3** (diseño de la app, `docs/design/`): W19 a W22. W19, W21 (con un `pliego-app.exe` de prueba) y W22 (cliente y servidor) pueden empezar ya; W20 tras W19.
- **Ola 4** (conectar las herramientas a la app): W23 a W29, ver [`ola4-conectar-herramientas.md`](ola4-conectar-herramientas.md).
- **Ola 5** (sustituir MuPDF por PDFium + lopdf tras la capa `pliego-pdf`; decisión de Ander 2026-11-04): W30 a W41, ver [`ola5-motor-propio.md`](ola5-motor-propio.md).

| Flujo | Tema | Ola | Depende de | Rutas propias |
| --- | --- | --- | --- | --- |
| W01 | App de escritorio y visor | 1 | base existente (pliego-core, pliego-engine-mupdf) | `app/` (frontend SolidJS en `app/src`, backend Tauri en `app/src-tauri`, que se añade como miembro del workspace). |
| W02 | Motor, aislamiento y guardado | 1 | base existente | `crates/pliego-engine-mupdf`, `crates/pliego-worker` (nuevo), `crates/pliego-document` (nuevo). |
| W03 | Anotaciones y rellenar formularios | 1 | base existente | `crates/pliego-annot`, `crates/pliego-forms`, `crates/pliego-cli/src/commands/{annot,form}.rs`. |
| W04 | Organizar páginas y sellos | 1 | base existente | `crates/pliego-organize`, `crates/pliego-stamp`, `crates/pliego-cli/src/commands/{pages,stamp}.rs`. |
| W05 | Firma digital PAdES y DNIe | 1 | base existente; coordinar con W02 para guardado incremental | `crates/pliego-sign`, `crates/pliego-cli/src/commands/sign.rs`, `tests/corpus/sign/`. |
| W06 | OCR | 1 | base existente | `crates/pliego-ocr`, `crates/tesseract-sys` si hace falta, `crates/pliego-cli/src/commands/ocr.rs`, `tools/` para scripts de construcción de Tesseract. |
| W07 | Proteger y redactar | 1 | base existente | `crates/pliego-protect`, `crates/pliego-redact`, `crates/pliego-cli/src/commands/{protect,redact}.rs`. |
| W08 | Comparar y optimizar | 1 | base existente | `crates/pliego-compare`, `crates/pliego-optimize`, `crates/pliego-cli/src/commands/{compare,optimize}.rs`. |
| W09 | Crear y convertir | 1 | base existente | `crates/pliego-convert`, `crates/pliego-cli/src/commands/convert.rs`. |
| W10 | Build, QA e integración con Windows | 1 | base existente; el instalador necesita W01 | `.github/`, `tools/`, `fuzz/`, `installer/`, `tests/corpus/README.md`, `benches/` (nuevo), `crates/pliego-shell` (nuevo, integración con el Explorador). |
| W11 | Edición de texto e imágenes | 2 | W02 (pliego-document) fusionado | `crates/pliego-edit`, `crates/pliego-cli/src/commands/edit.rs`. |
| W12 | PDF/A, PDF/UA y accesibilidad | 2 | base existente | `crates/pliego-pdfa`, `crates/pliego-a11y`, `crates/pliego-cli/src/commands/{pdfa,a11y}.rs`. |
| W13 | Diseñador de formularios | 2 | W03 fusionado | `crates/pliego-formdesign`, `crates/pliego-cli/src/commands/formdesign.rs`. |
| W14 | Exportar a Office | 2 | W09 (`structured_text`) fusionado | `crates/pliego-office-export`, `crates/pliego-cli/src/commands/office.rs`. |
| W15 | Acciones en lote, indice y medidas | 2 | módulos de la ola 1 fusionados | `crates/pliego-actions`, `crates/pliego-index`, `crates/pliego-measure`, `crates/pliego-portfolio`. |
| W16 | Producción de impresión | 2 | base existente | `crates/pliego-print`. |
| W17 | Flujo de firmas local | 2 | W05 fusionado | `crates/pliego-signflow`. |
| W18 | Asistente IA local | 2 | W01 fusionado | `crates/pliego-assistant`, `app/src/assistant/` (coordinar con W01). |
| W19 | Armazón de la interfaz y sistema de diseño | 3 | W01 (andamiaje Tauri) | `app/src/styles/`, `app/src/ui/`, `app/src/shell/`, `app/tests/visual/` (W01 conserva viewer, state, api, panels y src-tauri). |
| W20 | Navegación de herramientas, Inicio y Ajustes | 3 | W19 | `app/src/tools/`, `app/src/navigation/`, `app/src/home/`, `app/src/settings/`, `app/src/jobs/`, `app/src-tauri/src/commands/{settings,jobs,recents}.rs`. |
| W21 | Instalador Pliego Setup y lanzador | 3 | W01 (puede empezar con una app de prueba) | `crates/pliego-install`, `crates/pliego-launcher`, `crates/pliego-setup`, `installer/setup/` (W10 conserva el MSI, el ZIP y `.github/`). |
| W22 | Autoactualizaciones centralizadas | 3 | W21 y W20 para integrar; cliente y servidor ya | `crates/pliego-update`, `crates/pliego-release`, `app/src/updates/`, `app/src-tauri/src/commands/updates.rs`, `deploy/updates/`, `docs/releasing.md`. |

Archivos compartidos y reglas de propiedad: ver `docs/CONVENTIONS.md`.
