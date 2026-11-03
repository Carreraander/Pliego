# Integración en la app

Este documento lo comparten W01 (visor y estado), W19 (armazón) y W20 (navegación). Cada flujo edita solo su sección.

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
