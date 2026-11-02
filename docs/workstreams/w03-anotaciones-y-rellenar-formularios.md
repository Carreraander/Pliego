# W03 · Anotaciones y rellenar formularios

Ola 1. Depende de: base existente.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w03` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w03-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Rebase sobre `main` antes de abrir el PR; regenera `Cargo.lock`, no lo resuelvas a mano.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-annot`, `crates/pliego-forms`, `crates/pliego-cli/src/commands/{annot,form}.rs`.

## Objetivo
Comentar PDFs con todas las anotaciones estándar y rellenar cualquier formulario AcroForm, con intercambio de datos FDF/XFDF.

## Alcance
- Anotaciones: nota, texto libre, resaltado/subrayado/tachado/ondulado (por quads de texto), línea, flecha, rectángulo, elipse, polígono, polilínea, lápiz (ink), sello (predefinidos en español y personalizados con imagen), adjuntar archivo, enlace. Crear, editar propiedades (color, opacidad, grosor, autor, fecha, contenido), mover, borrar, responder (IRT) y estado (aceptado/rechazado).
- Generar apariencias (AP) para que se vean en cualquier visor. Aplanar anotaciones.
- Resumen de comentarios (lista estructurada exportable a texto/JSON; el PDF de resumen puede ir después).
- Importar/exportar comentarios XFDF.
- Formularios: listar campos (tipo, nombre, valor, opciones, flags), rellenar texto/casilla/radio/combo/lista, restablecer, aplanar, importar/exportar datos FDF, XFDF, JSON y CSV (para relleno masivo: un PDF por fila).
- Firma manuscrita simple: insertar imagen o trazo como anotación sellada (la firma con certificado es W05).

## Interfaces que debes exponer
- `pliego_annot::{list, add(AnnotSpec), update(id, AnnotPatch), delete(id), flatten, export_xfdf, import_xfdf, summary}` sobre `&mut mupdf::pdf::PdfDocument`.
- `pliego_forms::{fields, fill(FieldValues), reset, flatten, export_data(Format), import_data(Format), bulk_fill(template, csv, out_dir, ctx)}`.
- Tipos `AnnotSpec`/`FieldValue` serializables (los usará la app y las acciones en lote).
- CLI: `pliego annot list|add|delete|flatten|export|import`, `pliego form fields|fill|flatten|export|import|bulk`.

## Criterios de "hecho"
- Cada tipo de anotación se crea, se guarda y se vuelve a leer con las mismas propiedades (tests de ida y vuelta).
- Un PDF anotado por Pliego se ve igual en Acrobat Reader gratuito y en el navegador de acr12 (capturas en el PR).
- Rellenar y aplanar formularios de prueba, incluido uno oficial descargable de la AEAT o la Seguridad Social (no se sube al repo si no es redistribuible; se documenta la URL).
- `bulk_fill` genera N PDFs desde un CSV de N filas con progreso y cancelación.
