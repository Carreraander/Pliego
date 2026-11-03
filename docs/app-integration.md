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
`EmptyState`. Iconos por nombre (los de `ToolManifest.icon`): Lucide más los propios (`redact`, `bates`, `pdf-a`,
`ocr`, `timestamp`, `sign-digital`); un flujo puede añadir más con `registerIcon`.

### Catálogo
`npm run dev` y abrir `http://localhost:1420/ui-catalog` (solo en desarrollo): todos los componentes y estados,
con selector de tema y densidad. Las capturas de referencia están en `app/tests/visual/` (`npm run test:visual`,
claro, oscuro y alto contraste, con comprobación de accesibilidad con axe).
