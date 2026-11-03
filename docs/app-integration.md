# Integrar un módulo en la app de escritorio

La app tiene dos capas: `app/service` (`pliego-app-service`, sin Tauri, con la lógica y las pruebas) y
`app/src-tauri` (capa fina que expone comandos y el protocolo de teselas). El frontend vive en `app/src`.

## Añadir un comando Tauri
1. Crea `app/src-tauri/src/commands/<area>.rs` con tus funciones `#[tauri::command]`. Recibe el estado con
   `state: State<'_, AppState>` y devuelve `ApiResult<T>` (los `pliego_core::Error` se convierten solos en `ApiError`
   con `code` y `message`).
2. Declara `pub mod <area>;` en `commands/mod.rs` y añade tus funciones a `tauri::generate_handler![...]`.
3. Pon la lógica en tu crate de biblioteca, no en el comando: el comando solo traduce argumentos y errores.
4. Llámalo desde `app/src/api/` con `invoke("nombre", { ... })` (argumentos en camelCase).

## Documentos y teselas
- `open_document(path, password?) -> DocId`; los demás comandos reciben `docId`. Las páginas se numeran desde 0.
- Si tu módulo modifica un documento, llama a `bump_revision(docId)` (o `DocumentService::bump_revision`): sube la
  revisión, descarta las teselas en caché y emite el evento `document-changed { doc_id, revision }`. La interfaz
  debe volver a pedir las teselas al recibirlo.
- Teselas: `pliego://tile/<doc>/<página>/<zoom>/<col>/<fila>` (en Windows, `http://pliego.localhost/tile/...`),
  PNG de 512 px; zoom 1.0 = 72 ppp.

## Eventos
- `document-changed`: ver arriba.
- `open-files`: lista de rutas PDF enviadas por una segunda instancia (la app es de instancia única).
- Al arrancar, `startup_files` devuelve una vez los PDF pasados por línea de comandos.

## Menú, barra y paneles
El contrato de registro de herramientas (entrada de menú/barra, panel y comandos de cada módulo) lo define el
diseño de la app (hilo de diseño) y se documentará aquí cuando esté cerrado.
