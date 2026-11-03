# Convenciones de Pliego

## Licencia
- Todo el código es **AGPL-3.0-or-later**. Cada crate hereda `license.workspace = true`.
- Solo se admiten dependencias con licencias de `deny.toml`. Una licencia nueva (p. ej. GPL-3.0, LGPL) necesita
  aprobación del hilo de gestión en el PR.
- No copiar código de proyectos con licencia incompatible ni de Adobe. Inspirarse en el comportamiento, no en el código.
- Identidad propia: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Estructura y propiedad
- `crates/*` entra solo en el workspace (no hace falta editar el `Cargo.toml` raíz).
- Cada flujo es **dueño** de sus rutas (ver `docs/workstreams/README.md`) y solo edita esas.
- Archivos compartidos que cualquier flujo puede tocar, con cambios mínimos:
  - `crates/pliego-cli/src/main.rs`: una variante en `Command` y una línea en `match` por subcomando.
  - `crates/pliego-cli/src/commands/mod.rs`: una línea `pub mod <modulo>;`.
  - `crates/pliego-cli/Cargo.toml`: una dependencia a tu crate.
  - `Cargo.lock`: regenerar tras rebase, nunca resolver a mano.
- `pliego-core` y `pliego-engine-mupdf` son de gestión/W02. Si necesitas algo nuevo ahí, pídelo (mensaje al hilo
  de gestión) o abre un PR pequeño y separado solo con ese cambio.
- Los módulos de funciones **no dependen entre sí**: solo de `pliego-core` y de `mupdf` directamente.

## API de los módulos
- Biblioteca primero: funciones puras sobre rutas o sobre `mupdf::pdf::PdfDocument`, sin estado global.
- Operación larga = `fn op(..., ctx: &JobContext) -> pliego_core::Result<Report>`; llamar `ctx.step(..)` por página.
- Errores: `pliego_core::Error`; mensajes para personas, en español, sin jerga interna.
- Páginas desde 0 en la API, desde 1 en CLI e interfaz; rangos con `PageRange`.
- Nunca sobrescribir el archivo de entrada salvo que se pida explícitamente; escribir a un temporal y renombrar.
- Opciones con `serde::{Serialize, Deserialize}` para que la app y las acciones en lote puedan guardarlas.
- `unsafe_code = "deny"` en todo el workspace. Excepción: módulos `platform/windows.rs` o crates `*-sys` con
  `#![allow(unsafe_code)]`, cada bloque con comentario `// SAFETY:`, y revisión explícita en el PR.

## CLI
- Subcomando por módulo (`pliego pages ...`, `pliego sign ...`). Soporta `--json` global.
- Test de integración en `crates/pliego-cli/tests/<modulo>.rs` que ejecute el binario.

## Pruebas
- Unitarias junto al código; de integración en `tests/` del crate.
- PDFs de prueba pequeños y generados en `tests/corpus/<modulo>/` (con su script generador en `tools/` si es posible).
  Nada con datos personales reales. Archivos de más de 1 MB no se suben: se descargan con `tools/fetch_corpus`.
- Lo que solo se puede probar en Windows (CNG, COM de Office, Explorador) va con `#[cfg(windows)]` y se verifica en acr12.

## Ramas y PRs
- Rama `wNN-<tema-corto>` desde `main` actualizado. PRs pequeños (< 800 líneas de diff salvo código generado).
- Título en español, descripción con qué hace, cómo se probó (incluido acr12 si aplica) y qué falta.
- CI verde antes de pedir revisión. Para actualizar la rama, **merge de `main` en tu rama** (nunca rebase ni force-push); los PRs se fusionan con squash.
- Al abrir un PR, avisar al hilo de gestión con el enlace.

## Estilo
- Rust edición 2024, `rustfmt` por defecto (ancho 120, ver `rustfmt.toml`), `clippy -D warnings`.
- Comentarios y documentación en español; identificadores en inglés.
- Frontend: TypeScript estricto, SolidJS, sin frameworks CSS pesados; tokens de diseño en `app/src/styles/tokens.css`.
