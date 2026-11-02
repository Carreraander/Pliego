# W07 · Proteger y redactar

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
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w07` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w07-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Rebase sobre `main` antes de abrir el PR; regenera `Cargo.lock`, no lo resuelvas a mano.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-protect`, `crates/pliego-redact`, `crates/pliego-cli/src/commands/{protect,redact}.rs`.

## Objetivo
Proteger documentos (contraseñas, permisos, saneado) y redactar información de forma irreversible, con búsqueda de datos personales españoles.

## Alcance
- Cifrado AES-256 (y AES-128 para compatibilidad), contraseña de apertura y de permisos, permisos (imprimir, copiar, editar, comentar, rellenar, accesibilidad, ensamblar), quitar protección conociendo la contraseña.
- Cifrado por certificado (destinatarios con clave pública, /PubSec) si MuPDF lo permite; si no, documentar y dejar para fase 2.
- Sanear: quitar metadatos (Info y XMP), JavaScript, acciones de apertura, adjuntos, archivos incrustados, capas ocultas, comentarios, datos de formularios, miniaturas y objetos sin referencia.
- Redacción: marcar áreas o texto, revisar marcas y aplicar con eliminación real de texto, imágenes (o la parte cubierta) y vectores debajo; color y texto de relleno opcional.
- Buscar y redactar por patrones: DNI/NIE (con letra de control), CIF, IBAN (con dígitos de control), email, teléfono español, fechas, y expresiones regulares propias; vista previa de coincidencias antes de aplicar.
- Verificación posterior: el texto redactado no se puede extraer ni buscar.

## Interfaces que debes exponer
- `pliego_protect::{encrypt(Encryption), decrypt(password), set_permissions, sanitize(SanitizeOptions) -> Report}`.
- `pliego_redact::{find_patterns(doc, &[Pattern]) -> Vec<Match>, mark(areas), apply(RedactOptions) -> Report, verify(doc, &[String])}`.
- CLI: `pliego protect encrypt|decrypt|permissions|sanitize`, `pliego redact --pattern dni,iban,email --apply`.

## Criterios de "hecho"
- PDF cifrado por Pliego se abre en Acrobat Reader con la contraseña y respeta los permisos (captura en el PR).
- Tras redactar, `pliego text` y la búsqueda no encuentran ninguna cadena redactada (test por cada patrón) y las imágenes cubiertas quedan modificadas.
- Validadores de DNI/NIE/CIF/IBAN con tests de casos válidos e inválidos.
- `sanitize` deja un informe de lo eliminado.
