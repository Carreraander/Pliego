# W13 · Diseñador de formularios

Ola 2. Depende de: W03 fusionado.

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w13` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w13-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Rebase sobre `main` antes de abrir el PR; regenera `Cargo.lock`, no lo resuelvas a mano.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-formdesign`, `crates/pliego-cli/src/commands/formdesign.rs`.

## Objetivo
Crear formularios: añadir y editar campos, detección automática de campos y lógica de cálculo y validación.

## Alcance
- Crear/editar/borrar campos de texto, casilla, radio, combo, lista, botón y firma; propiedades (nombre, obligatorio, solo lectura, formato, máscara, valor por defecto, orden de tabulación).
- Detección automática de campos en PDFs planos y escaneos: líneas, cajas y etiquetas cercanas (heurísticas; modelo ONNX opcional).
- Cálculos (suma, producto, media, mín., máx. de campos) y validación por rango y formato; JavaScript AcroForm compatible con lo habitual en formularios españoles, ejecutado de forma aislada (mujs de MuPDF).

## Interfaces que debes exponer
- `pliego_formdesign::{add_field(FieldSpec), update_field, delete_field, detect_fields(page, ctx) -> Vec<FieldSpec>, set_calculation, set_validation, set_tab_order}`.

## Criterios de "hecho"
- La detección encuentra al menos el 80 % de los campos en 10 formularios planos de prueba.
- Formularios creados funcionan (cálculos y validaciones) en Acrobat Reader y en Pliego.
