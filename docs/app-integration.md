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
Las herramientas se declaran en el registro del frontend (sección «Herramientas», más abajo); el armazón y los
paneles, con las ranuras de la sección «Interfaz».

## Interfaz

> Sección de W19. Diseño de referencia: `docs/design/` (`ux-y-navegacion.md`, `sistema-visual.md`, `tokens.css`).

### Reglas
- Ningún flujo crea estilos propios para botones, campos, menús, paneles de tarea, etc.: se usan los componentes de
  `app/src/ui/` (`import { Button, Field, … } from "@/ui"` o ruta relativa a `ui/index`).
- Ningún color literal fuera de `app/src/styles/tokens.css`: todo con `var(--pl-*)`. Lo comprueba
  `app/src/ui/no-literal-colors.test.ts`.
- Tema, densidad y alto contraste salen de los tokens (`data-theme`, `data-density`, `forced-colors`); no hay que
  hacer nada en cada componente.
- Textos: claves en el diccionario del área (`app/src/i18n`); los textos genéricos de componentes viven en
  `app/src/ui/strings.ts`.

### Componentes (`app/src/ui/`)
`Icon`, `Button` (`primary`, `normal`, `ghost`, `danger`), `IconButton` (nombre accesible obligatorio), `Field` +
`TextInput`/`Select`/`Slider`, `Checkbox`, `Switch`, `RadioGroup`/`Radio`, `ToggleGroup`, `Kbd`, `Chip`, `Notice`,
`EmptyState`, `Popover`, `Tooltip` (con atajo), `Menu`/`ContextMenu`/`MenuButton` (teclado completo, submenús, entradas desactivadas con motivo), `Tabs`/`TabPanel`, `Toolbar`, `toast()` + `ToastRegion`, `Inspector`, `TaskPanel` (título, opciones, avanzado plegado, estimación, pie con acciones y progreso), `ProgressBar`, `WizardSteps`. Iconos por nombre (los de `ToolManifest.icon`): Lucide más los propios (`redact`, `bates`, `pdf-a`,
`ocr`, `timestamp`, `sign-digital`); un flujo puede añadir más con `registerIcon`.

### Catálogo
`npm run dev` y abrir `http://localhost:1420/ui-catalog` (solo en desarrollo): todos los componentes y estados,
con selector de tema y densidad. Las capturas de referencia están en `app/tests/visual/` (`npm run test:visual`,
claro, oscuro y alto contraste, con comprobación de accesibilidad con axe).

### Armazón y ranuras (`app/src/shell/`)
`AppWindow` (barra de título con pestañas, barra de modos, raíl, panel lateral, lienzo, inspector, barra de estado) lo
monta `App.tsx`. Nadie más edita `shell/`: cada flujo se registra con `app/src/shell/slots.ts` (`import … from "../shell"`):

| API | Para qué |
| --- | --- |
| `registerPanel({ id, title, icon, order, shortcut, component })` | Pestaña en el raíl y su contenido en el panel lateral. El componente recibe `{ docId }`. Devuelve la función para retirarlo. |
| `openInspector(Componente, props)` / `closeInspector()` | Contenido del inspector (propiedades o `TaskPanel`). El componente recibe además `onClose`. `F9` lo oculta y recupera. |
| `showNotice(docId, { kind, tone, text, actions })` | Barra de aviso del documento. Una sola visible, por prioridad: `repaired` > `signed` > `protected` > `form` > `pdfa` > `restricted`. |
| `registerStatusItem({ id, slot: "start"\|"center"\|"end", text \| component, onClick, tone })` | Elemento de la barra de estado. |
| `registerMode({ id, title, icon, toolbar })` | Modo de la barra de modos y su barra de herramientas (`Alt+1…9`). |
| `registerTitleItem({ id, component })` | Hueco de la barra de título (pastilla de actualización de W22). |
| `setPaletteOpener(fn)` / `setToolsOpener(fn)` | La paleta de órdenes (`Ctrl+K`) y la de herramientas (`Ctrl+Mayús+A`) las implementa W20. |

Los ids son `<area>.<nombre>`. Los textos ya van traducidos (`t(...)`). Atajos del armazón: `F4` panel lateral, `F6` /
`Mayús+F6` recorren regiones (título → modos → raíl → panel → lienzo → inspector → estado), `F9` inspector, `Alt+1…9` modos,
`Ctrl+K` paleta. Responsivo: < 1100 px los modos se pliegan a un desplegable; < 900 px panel e inspector se superponen.
Demo con contenido de ejemplo en desarrollo: `http://localhost:1420/ui-shell`.

### Iconos de la app
`npm run icons` (`app/scripts/gen-icons.mjs`) genera `app/src-tauri/icons/` (`icon.ico` 16–256 px, PNG y `logo.svg`) con el logo.

## Herramientas
> Sección de W20. Contrato de diseño: `docs/design/ux-y-navegacion.md` §10.

La barra de modos, la paleta «Herramientas» (`Ctrl+Mayús+A`), la paleta de órdenes (`Ctrl+K`), los atajos de una letra y
el menú contextual se generan del **registro de herramientas** (`app/src/tools/registry.ts`). Para añadir una herramienta
no se toca el armazón ni `navigation/`: se crea una carpeta `app/src/tools/<area>/` con un manifiesto y un panel, y se
añade **una línea** de registro.

### Qué toca cada flujo
1. `app/src/tools/<area>/manifest.ts`: el `ToolManifest` (con el **mismo `id`** que la herramienta «fantasma» que ya
   aparece desactivada en la paleta; al registrarse la sustituye).
2. `app/src/tools/<area>/<Panel>.tsx`: el panel de tarea (un componente con `TaskPanel`) o el componente de la
   pantalla. Se carga con `load: () => import("./<Panel>")`, de forma perezosa.
3. `app/src/api/…` y `app/src-tauri/src/commands/<area>.rs`: el comando Tauri (ver «Añadir un comando Tauri»).
4. Una línea en `app/src/tools/index.ts`: `registerTool(compress);` (o en tu propio `register<Area>Tools()`).
5. Textos: `addToolStrings({ es: { "tools.optimize.compress.title": "Comprimir" }, en: { … } })`.

### `ToolManifest`
| Campo | Significado |
| --- | --- |
| `id` | Único, `<area>.<herramienta>`. Los ids de la tabla de §4.3 ya existen como fantasmas (`app/src/tools/base.ts`). |
| `group` | Grupo de la paleta (`view`, `comment`, `fill-sign`, `organize`, `edit`, `forms`, `scan-ocr`, `protect`, `convert`, `compare-optimize`, `standards`, `print`, `automate`, `assistant`). |
| `kind` | `mode` (cambia la barra del modo; indica `modeId` si el modo se registró con otro id), `task` y `wizard` (el `load` devuelve el panel del inspector), `panel` (indica `panelId` del raíl), `screen` (lienzo completo). |
| `title`, `description` | Claves del diccionario de herramientas (`addToolStrings`). |
| `icon`, `keywords`, `shortcut` | Icono por nombre, palabras para buscar y atajo (una letra solo vale con el foco en el lienzo; los demás son globales). El gestor avisa de los conflictos y el usuario puede cambiarlo en Ajustes. |
| `when` | Requisitos: `document`, `forms`, `signatures`, `protected`, `pdfa`. Si no se cumplen la herramienta aparece desactivada con el motivo. Los hechos del documento los fija quien los conoce con `setDocFacts({ forms: true })`. |
| `pinnable` | Se puede fijar en la barra de modos (clic derecho en la paleta). |
| `load` | Importación perezosa del componente; recibe `{ toolId, onClose }`. |

### Ejemplo completo: Comprimir (`pliego-optimize`)
```ts
// app/src/tools/optimize/manifest.ts
import type { ToolManifest } from "../registry";
export const compress: ToolManifest = {
  id: "optimize.compress", group: "compare-optimize", kind: "task",
  title: "tools.optimize.compress.title", description: "tools.optimize.compress.description",
  icon: "file-down", keywords: ["reducir", "tamaño", "optimizar"], when: { document: true }, pinnable: true,
  load: () => import("./CompressPanel"),
};
```
```tsx
// app/src/tools/optimize/CompressPanel.tsx
import { createSignal } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { Radio, RadioGroup, TaskPanel, toast } from "../../ui";
import { activeId } from "../../state/documents";

export default function CompressPanel(p: { onClose: () => void }) {
  const [quality, setQuality] = createSignal("balanced");
  const apply = async () => {
    await invoke("compress_pdf", { docId: activeId(), quality: quality() }); // devuelve un JobId (ver «Tareas»)
    toast({ tone: "info", text: "Comprimiendo… sigue en el centro de tareas" });
    p.onClose();
  };
  return (
    <TaskPanel title="Comprimir" onClose={p.onClose} primary={{ label: "Guardar copia", onClick: apply }}>
      <RadioGroup legend="Calidad" value={quality()} onChange={setQuality}>
        <Radio value="max" label="Máxima calidad" /><Radio value="balanced" label="Equilibrado" /><Radio value="min" label="Mínimo tamaño" />
      </RadioGroup>
    </TaskPanel>
  );
}
```
```ts
// app/src/tools/index.ts: una línea
registerTool(compress);
```
El resultado se ve en `http://localhost:1420/nav-demo` (solo desarrollo): `Ctrl+K` → «comprimir».

### Paleta de órdenes y atajos
- Órdenes sueltas: `registerCommand({ id, title, icon, shortcut?, run })`. Fuentes nuevas (recientes, ajustes…):
  `registerCommandProvider({ id, order, group, query(q) })`. La búsqueda es aproximada, sin tildes ni mayúsculas
  (`navigation/fuzzy.ts`).
- Atajos: `registerShortcut({ id, section, keys, title, run, scope })` (`navigation/shortcuts.ts`). Es la única tabla:
  no escuches `keydown` por tu cuenta. Los atajos que cambie el usuario se guardan en los ajustes.
- Pantallas a lienzo completo: `openScreen({ id, component })` / `closeScreen()`.
- Ajustes: `app/src/settings/store.ts` (`settings()`, `updateSettings(sección, cambios)`); esquema versionado.

## Tareas en segundo plano
Una operación larga (comprimir, OCR, convertir…) corre como **trabajo** y aparece en el centro de tareas (popover de la
barra de estado). El registro vive en `pliego-appdata` (`jobs.rs`, con pruebas) y se expone a Tauri como `JobsState`
(`app/src-tauri/src/commands/jobs.rs`).

```rust
// app/src-tauri/src/commands/optimize.rs
#[tauri::command]
pub fn compress_pdf(jobs: State<'_, JobsState>, path: String, quality: String) -> JobId {
    // `spawn` devuelve el JobId de inmediato y ejecuta el cierre en un hilo. `ctx` es el JobContext de pliego-core:
    // `ctx.step(hecho, total, "texto")?` emite `job-progress` y devuelve Err(Cancelled) si se canceló.
    jobs.0.spawn(move |ctx| pliego_optimize::compress(&path, &quality, ctx))
}
```
```ts
// en el panel de la tarea
const id = await invoke<number>("compress_pdf", { path, quality });
trackJob(id, { title: "Comprimir contrato.pdf", retry: apply }); // import { trackJob } from "../../jobs"
```
- Eventos: `job-progress { job_id, done, total, label }` y `job-finished { job_id, result | error }` (`result` es el
  `Report` de `pliego-core`: `outputs`, `warnings`, `summary`). Cancelar: `cancel_job(job_id)`.
- El centro de tareas ofrece cancelar, «Abrir resultado» (primer PDF de `outputs`), «Mostrar en carpeta» y
  «Reintentar» (si pasaste `retry`). Con la ventana en segundo plano al terminar, lanza una notificación de Windows.
- Si la operación modifica el documento abierto, llama a `bump_revision(docId)` al terminar (ver «Documentos»).

## Registro de diagnóstico

`pliego_core::log` (`info`, `warn`, `error`) escribe en `%APPDATA%\Pliego\logs\pliego.log` solo si la app lo activó
(la CLI y las pruebas no escriben nada). Rota a 512 KiB y conserva 5 archivos (2,5 MiB como máximo). Qué se anota:
arranque (versión, sistema, carpeta de datos), fallos del worker (caída, tiempo agotado, relanzado), documentos que no se
abren, trabajos que fallan (no los cancelados), pánicos y todo el ciclo de actualizaciones. **Nunca** contenidos de
documentos ni contraseñas: solo mensajes de error del motor; la carpeta personal se sustituye por `~`. Si añades un
`log::…` nuevo, pasa un mensaje de error o un evento, no datos del usuario. Ajustes > Avanzado tiene «Abrir carpeta de
registros» (`open_logs_folder`).

## Ajustes, recientes e Inicio
- `settings()` / `updateSettings("sección", { campo: valor })` (`app/src/settings/store.ts`): esquema versionado con
  efecto inmediato; en Windows se guarda en `%APPDATA%\Pliego\settings.json` (`get_settings`/`set_settings`). Tu flujo
  puede añadir su sección a Ajustes con `registerSettingsSection({ id, title, icon, rows | component, keywords })`
  (Comentarios, Firma, OCR e idiomas, Privacidad… las añade cada dueño).
- Las autoactualizaciones (W22) sustituyen el servicio simulado de Ajustes > Actualizaciones con `setUpdateService(...)`.
- Recuperación tras un cierre inesperado (W02): al arrancar, `setRecoverable(docs, { restore, discard })` (`app/src/home`)
  muestra en Inicio «Pliego se cerró sin guardar N documentos».
- Recientes: `recordOpened(path)` ya se llama al abrir un PDF; `recents_*` y `recent_thumbnail` son los comandos.
