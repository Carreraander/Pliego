# Flujos de trabajo

Cada archivo `wNN-*.md` es un prompt autocontenido para una sesión de implementación. La gestión (revisión de PRs,
interfaces compartidas, prioridades) la lleva el hilo de gestión.

- **Ola 1** (en paralelo desde ya): W01 a W10.
- **Ola 2**: W11 a W18, cada uno cuando su dependencia esté fusionada en `main`.

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

Archivos compartidos y reglas de propiedad: ver `docs/CONVENTIONS.md`.
