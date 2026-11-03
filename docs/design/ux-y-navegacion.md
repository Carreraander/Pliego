# Pliego · UX y navegación

Estado: propuesta v1 (2026-11-03), pendiente de aprobación de Ander. Prototipo navegable: `docs/design/prototipo.html`.
Sustituye al boceto del PR #1.

## 1. Principios

1. **El documento manda.** La interfaz ocupa lo mínimo; el lienzo es lo más grande y lo más tranquilo de la pantalla.
   Nada de colores fuertes en el cromo salvo el acento en lo que está activo.
2. **Todo está a un atajo o a una búsqueda.** La paleta de órdenes (`Ctrl+K`) encuentra cualquier herramienta,
   ajuste, documento reciente o página. Ninguna función queda escondida solo en un menú.
3. **Nada destructivo sin red.** Deshacer en todo; las tareas que reescriben el archivo (optimizar, OCR, PDF/A,
   proteger, redactar) guardan por defecto una copia; lo irreversible (aplicar redacción) se confirma dentro del panel,
   nunca con un diálogo que se acepta sin leer.
4. **Sin modales que bloqueen.** Las opciones de una tarea viven en el inspector de la derecha y el trabajo largo corre
   en segundo plano con progreso en la barra de estado. Se puede seguir leyendo mientras se hace OCR de otro documento.
5. **Se nota que es Windows.** Fuente del sistema, Mica en la barra de título, Snap Layouts en el botón maximizar,
   tema y escala de texto de Windows, menú contextual con clic derecho, arrastrar y soltar desde el Explorador.
6. **Identidad propia.** Ni nombres, ni iconos, ni disposición de Acrobat. Vocabulario propio y llano
   ("Organizar", "Tapar datos" no; "Redactar" sí porque es el término técnico extendido; ver §9).

## 2. Anatomía de la ventana

```
┌──────────────────────────────────────────────────────────────────────────────────────────────┐
│ [◩ Pliego ▾] [contrato.pdf •  ×] [factura.pdf ×] [+]      [⌕ Buscar o ejecutar…  Ctrl+K] [⟳] ─ □ ×│  barra de título 40
├──────────────────────────────────────────────────────────────────────────────────────────────┤
│ Leer  Comentar  Rellenar y firmar  Organizar  Editar  Proteger  Herramientas ▾ │ [barra del modo] │  barra de modos 44
├────┬───────────────┬──────────────────────────────────────────────────────┬──────────────────┤
│ ▤  │ Miniaturas    │ ⚠ aviso del documento (firmado, protegido, reparado) │ Inspector        │
│ ☰  │  ┌──┐ ┌──┐    │                                                      │ (propiedades de  │
│ 🔖 │  │1 │ │2 │    │                 ┌──────────────┐                      │  la selección o  │
│ 📎 │  └──┘ └──┘    │                 │              │                      │  panel de tarea) │
│ 💬 │               │                 │    página    │       [− 100 % +]    │                  │
│ ⧉  │               │                 └──────────────┘                      │                  │
├────┴───────────────┴──────────────────────────────────────────────────────┴──────────────────┤
│ Pág. 2 de 48 · A4 210 × 297 mm   │  ◷ OCR de informe.pdf 42 %   │  ✓ Firmado · válido │ 100 % ▾ │  barra de estado 28
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

| Zona | Medida | Contenido | Notas |
| --- | --- | --- | --- |
| Barra de título | 40 px, Mica | Menú de la app (logo), pestañas, buscador/paleta, indicador de actualización, botones de ventana | Pestañas arrastrables y reordenables; arrastrar una fuera abre ventana nueva. Doble clic en hueco = maximizar. Botón maximizar con Snap Layouts (plugin `tauri-plugin-decorum` o equivalente, verificar licencia). |
| Barra de modos | 44 px | Izquierda: modos. Derecha: herramientas del modo activo | En ventanas estrechas (< 1100 px) los modos se pliegan a un selector desplegable y la barra del modo pasa a iconos. |
| Raíl | 48 px | Paneles: Miniaturas, Marcadores, Comentarios, Adjuntos, Capas, Firmas, Búsqueda | Clic en el activo pliega el panel. `F4` alterna panel. |
| Panel lateral | 264 px (200–480) | El panel elegido en el raíl | Redimensionable, recuerda ancho por panel. |
| Lienzo | resto | Páginas por teselas, zoom flotante abajo a la derecha | Fondo `--pl-canvas`. Barra de aviso del documento pegada arriba del lienzo. |
| Inspector | 320 px (280–480) | Propiedades de lo seleccionado o el panel de la tarea en curso | Se abre solo cuando hay algo que mostrar; `F9` alterna. |
| Barra de estado | 28 px | Página y tamaño de papel, centro de tareas, estado del documento (firma, protección, PDF/A), zoom y modo de vista | Cada elemento es clicable (ir a página, abrir centro de tareas, panel de firmas…). |

Ventana mínima 720 × 480. Por debajo de 900 px de ancho el panel lateral y el inspector se superponen al lienzo en
lugar de empujarlo.

## 3. Pantallas

### 3.1 Inicio (sin documento, o pestaña `+`)
- Saludo corto y zona de soltar archivos ("Suelta aquí un PDF o haz clic para abrir").
- **Acciones rápidas** (6 tarjetas, personalizables): Abrir, Combinar archivos, Escanear y reconocer texto (OCR),
  Firmar, Comprimir, Convertir. Todas aceptan archivos soltados encima.
- **Recientes**: lista con miniatura, nombre, carpeta, fecha y estado (firmado, protegido). Fijar arriba con estrella.
  Clic derecho: abrir ubicación, quitar de la lista. Si el archivo ya no existe, se muestra atenuado con "Buscar".
- **Todas las herramientas**: enlace a la paleta de herramientas (§4.3).
- Pie discreto: versión, "Software libre · AGPL-3.0".

### 3.2 Documento
La anatomía del §2. Al abrir: modo **Leer**, panel de miniaturas abierto si el documento tiene más de 1 página,
zoom "ajustar ancho" si la página es vertical, "ajustar página" si es apaisada (configurable).

### 3.3 Comparar (pantalla especial)
Dos documentos lado a lado con desplazamiento sincronizado, lista de cambios en el inspector (texto añadido, eliminado,
movido; diferencias visuales), filtro por tipo y botón "Generar informe PDF". Se abre desde Herramientas o soltando dos
archivos en la tarjeta "Comparar".

### 3.4 Organizar (sustituye al lienzo)
Rejilla de páginas grandes (tamaño ajustable con `Ctrl+rueda`), selección múltiple con `Ctrl`/`Mayús`, arrastrar para
reordenar, soltar archivos entre páginas para insertarlos. Barra del modo: Insertar, Eliminar, Rotar ⟲ ⟳, Extraer,
Dividir, Recortar, Duplicar, Numerar páginas. Separadores visuales entre documentos combinados.

### 3.5 Ajustes (pestaña especial, no ventana)
Secciones a la izquierda: General, Visor, Comentarios, Firma, OCR e idiomas, Actualizaciones, Privacidad, Accesibilidad,
Avanzado, Acerca de. Cambios con efecto inmediato (sin botón Guardar). Buscador de ajustes arriba.
Detalle de Actualizaciones en `actualizaciones.md` §6.

### 3.6 Centro de tareas
Popover desde la barra de estado: tareas en curso (barra de progreso, cancelar), terminadas (abrir resultado, mostrar
en carpeta) y fallidas (mensaje claro y "Reintentar"). Las tareas siguen aunque se cierre la pestaña del documento.
Al terminar una tarea larga con la ventana en segundo plano: notificación de Windows.

## 4. Navegación de herramientas

### 4.1 Modelo: modos + tareas + asistentes
Cada herramienta es de uno de tres tipos. Esto define cómo se ve y cómo se comporta:

| Tipo | Qué es | Dónde vive | Ejemplos |
| --- | --- | --- | --- |
| **Modo** | Forma de trabajar continua sobre el documento | Cambia la barra del modo y el inspector; se mantiene hasta cambiar de modo | Leer, Comentar, Rellenar y firmar, Organizar, Editar, Proteger, Formularios, Redactar, Accesibilidad, Impresión |
| **Tarea** | Operación con opciones que produce un resultado | Panel de tarea en el inspector: opciones → vista previa del efecto → "Aplicar" o "Guardar copia". Corre como trabajo en segundo plano | Comprimir, OCR, Proteger con contraseña, Convertir, Exportar, PDF/A, Combinar, Dividir, Marca de agua |
| **Asistente** | Proceso de varios pasos con decisiones | Hoja de pasos en el inspector (o centrada si no hay documento) con "Atrás / Siguiente" | Firma digital, Flujo de firmas, Acciones en lote, Crear índice |

Regla: una tarea nunca abre un modal. La única excepción son las preguntas del sistema (contraseña de apertura,
PIN del DNIe, que pide Windows) y las confirmaciones de cerrar con cambios sin guardar.

### 4.2 Modos principales (barra de modos)
Seis fijos por defecto + "Herramientas". El usuario puede fijar cualquier herramienta como modo y reordenar
(clic derecho → "Fijar en la barra"). `Alt+1`…`Alt+9` cambian de modo según su posición.

| Modo | Barra del modo (de izquierda a derecha) | Inspector | Flujo |
| --- | --- | --- | --- |
| **Leer** | Seleccionar `V`, Mano `H`, Zoom ▾, Vista (continua, página, doble, libro) ▾, Rotar vista, Leer en voz alta, Presentación `F11` | Propiedades del documento (si se pide) | W01 |
| **Comentar** | Resaltar `H`, Subrayar `U`, Tachar `K`, Nota `N`, Texto `T`, Dibujo `P`, Formas ▾ (rect., elipse, línea, flecha, polígono, nube) `R`, Sello ▾ `S`, Adjuntar archivo, Color, Grosor | Propiedades de la anotación seleccionada (color, opacidad, autor, estado, respuestas) | W03 (sellos W04) |
| **Rellenar y firmar** | Rellenar campos (resalta campos), Texto libre, ✓ ✗ •, Fecha, Mi firma ▾ (dibujada/imagen/escrita), **Firma digital…**, Validar firmas, Pedir firmas… | Campo seleccionado / asistente de firma | W03, W05, W17 |
| **Organizar** | Insertar ▾, Eliminar, Rotar ⟲ ⟳, Extraer, Dividir…, Recortar, Duplicar, Numerar páginas, Combinar con… | Propiedades de la selección (n.º de páginas, tamaño) | W04 |
| **Editar** | Editar texto `E`, Imagen ▾, Enlace, Encabezado y pie…, Marca de agua…, Fondo…, Bates… | Fuente, tamaño, color, alineación; propiedades de imagen | W11 (marca de agua/Bates con W04) |
| **Proteger** | Contraseña…, Permisos…, Cifrar con certificado…, Redactar ▸ (marcar, buscar datos personales, aplicar), Sanear documento…, Quitar protección | Panel de la tarea | W07 |

### 4.3 Paleta "Herramientas" (todas)
Panel grande desplegado desde "Herramientas ▾" (o `Ctrl+Mayús+A`): buscador arriba y herramientas por grupo,
cada una con icono, nombre y una línea de descripción. Las desactivadas (p. ej. "Validar firmas" sin firmas) se muestran
atenuadas con el motivo en el tooltip, no se ocultan.

| Grupo | Herramientas (tipo) | Flujo |
| --- | --- | --- |
| Ver | Leer (modo), Presentación (modo), Capas (panel), Adjuntos (panel), Propiedades del documento (tarea) | W01 |
| Comentar | Comentar (modo), Sellos (modo), Importar/exportar comentarios XFDF (tarea), Resumen de comentarios (tarea) | W03, W04 |
| Rellenar y firmar | Rellenar y firmar (modo), Firma digital (asistente), Validar firmas (panel), Flujo de firmas (asistente) | W03, W05, W17 |
| Organizar | Organizar (modo), Combinar archivos (tarea), Dividir (tarea), Insertar páginas (tarea), Numerar páginas (tarea) | W04 |
| Editar | Editar (modo), Marca de agua (tarea), Encabezado y pie (tarea), Fondo (tarea), Bates (tarea) | W11, W04 |
| Formularios | Diseñar formulario (modo), Detectar campos (tarea) | W13 |
| Escanear y OCR | Reconocer texto (tarea), Enderezar y limpiar (tarea), Idiomas de OCR (ajustes) | W06 |
| Proteger | Proteger (modo), Redactar (modo), Sanear (tarea) | W07 |
| Convertir | Crear PDF (tarea), Exportar a imágenes/texto (tarea), Exportar a Word/Excel/PowerPoint (tarea, "beta"), Guardar como PDF/A (tarea) | W09, W14, W12 |
| Comparar y optimizar | Comparar (pantalla), Comprimir (tarea), Optimizar avanzado (tarea) | W08 |
| Estándares y accesibilidad | Comprobar accesibilidad (modo), Orden de lectura (modo), Validar PDF/A (tarea) | W12 |
| Impresión | Vista previa de salida (modo), Comprobación previa (tarea) | W16 |
| Automatizar | Acciones en lote (asistente), Índice de búsqueda (asistente), Medir (modo), Carteras (modo) | W15 |
| Asistente | Asistente IA local (panel derecho) | W18 |

### 4.4 Paleta de órdenes (`Ctrl+K`)
Un único campo en la barra de título. Busca, en este orden: órdenes y herramientas, documentos abiertos, recientes,
"ir a página 12" (números), texto dentro del documento ("buscar «cláusula»"), ajustes. Coincidencia aproximada sin
tildes ni mayúsculas, con atajo visible a la derecha de cada resultado. `Enter` ejecuta, `Tab` amplía (p. ej. abre
opciones de la tarea en el inspector). Es también la forma accesible de llegar a todo.

### 4.5 Menú de la app (logo)
Nuevo ▸ (desde archivos, desde portapapeles, desde escáner), Abrir… `Ctrl+O`, Recientes ▸, Guardar `Ctrl+S`,
Guardar como… `Ctrl+Mayús+S`, Exportar ▸, Imprimir… `Ctrl+P`, Propiedades del documento `Ctrl+D`, Ajustes `Ctrl+,`,
Ayuda y atajos `F1`, Buscar actualizaciones, Acerca de Pliego, Salir.

### 4.6 Clic derecho en el lienzo
Depende del contexto: sobre texto seleccionado (Copiar, Resaltar, Subrayar, Tachar, Comentar, Buscar, Redactar);
sobre una anotación (Propiedades, Responder, Estado, Eliminar); sobre la página (Rotar, Insertar página, Extraer,
Imprimir esta página).

## 5. Patrones de interacción

### 5.1 Panel de tarea (inspector)
```
┌ Comprimir ─────────────────── × ┐
│ Calidad   ( ) Máxima calidad     │
│           (•) Equilibrado        │
│           ( ) Mínimo tamaño      │
│ ▸ Opciones avanzadas             │
│ ──────────────────────────────── │
│ Ahora 12,4 MB → estimado ~3,1 MB │
│ [Guardar copia]  [Reemplazar…]   │
└──────────────────────────────────┘
```
- Título, cierre, opciones simples arriba y "Opciones avanzadas" plegadas.
- Siempre que sea posible, estimación o vista previa del resultado antes de aplicar.
- Botón primario = la opción segura (guardar copia como `nombre (comprimido).pdf` junto al original).
  "Reemplazar el original" es secundario y pide confirmación dentro del propio panel.
- Al aplicar, el panel muestra el progreso y se puede cerrar: la tarea pasa al centro de tareas.
- Opciones serializables: "Guardar como ajuste" las guarda con nombre para reutilizarlas (y para Acciones en lote).

### 5.2 Barra de aviso del documento
Franja de 36 px arriba del lienzo, una sola a la vez y con prioridad: reparado > firmado > protegido > formulario >
PDF/A > modo restringido (worker aislado). Texto llano + una acción: "Firmado por ANA GARCÍA · firma válida ·
Ver firmas"; "Este PDF tiene campos para rellenar · Resaltar campos"; "Se reparó al abrir · Guardar versión reparada".

### 5.3 Editar un documento firmado o PDF/A
Antes del primer cambio que invalide la firma o la conformidad, aviso en línea en la barra de aviso: "Si cambias este
documento, la firma de ANA GARCÍA dejará de ser válida. [Guardar como copia y editar] [Editar igualmente]".
Las anotaciones permitidas por DocMDP no avisan.

### 5.4 Redacción (dos pasos)
1. Marcar: las zonas marcadas se ven con trama roja translúcida (`--pl-redact-mark`) y se listan en el inspector.
   Buscar datos: DNI/NIE, CIF, IBAN, correo, teléfono, patrones propios.
2. Aplicar: resumen ("Se eliminarán 14 zonas en 6 páginas. No se puede deshacer en el archivo resultante") y botón
   "Aplicar y guardar copia". Tras aplicar, verificación automática (texto extraído ya no contiene lo redactado).

### 5.5 Firma digital (asistente, 4 pasos)
1. Dónde: dibujar el recuadro de la firma visible o "firma invisible".
2. Con qué: certificado (lista del almacén de Windows con DNIe/FNMT detectados; archivo .p12/.pfx), aspecto
   (nombre, fecha, motivo, logotipo).
3. Opciones: nivel PAdES (B-B, B-T con sello de tiempo, B-LT, B-LTA), servidor de sellos de tiempo, bloquear documento.
4. Firmar: Windows pide el PIN; guardar como copia firmada por defecto. Resultado con validación inmediata.

### 5.6 Deshacer, guardar y recuperación
- `Ctrl+Z`/`Ctrl+Y` por documento, con historial en el menú del documento ("Deshacer «Rotar páginas 3–5»").
- Pestaña con cambios sin guardar: punto `•` en lugar de la × hasta pasar el ratón.
- Autoguardado de recuperación cada 2 min (W02); al reabrir tras un cierre inesperado: "Pliego se cerró sin guardar
  2 documentos. [Recuperar] [Descartar]" en la pantalla de inicio.

### 5.7 Errores
Mensajes de persona, en la zona donde ocurrió, con causa y solución: "No se pudo abrir «informe.pdf»: el archivo está
dañado y no se puede reparar. Prueba a descargarlo de nuevo." Nunca códigos internos en la superficie (sí en
"Detalles" plegado para copiar).

### 5.8 Arrastrar y soltar
- Sobre la barra de pestañas: abrir. Sobre el lienzo de un documento: menú "Abrir en pestaña nueva / Insertar
  después de la página actual / Adjuntar al PDF". Sobre una tarjeta de Inicio: ejecutar esa acción con esos archivos.
- Desde Pliego hacia el Explorador: arrastrar una miniatura extrae esas páginas como PDF.

## 6. Atajos de teclado (por defecto)

| Ámbito | Atajos |
| --- | --- |
| Archivo | `Ctrl+O` abrir, `Ctrl+S` guardar, `Ctrl+Mayús+S` guardar como, `Ctrl+P` imprimir, `Ctrl+W` cerrar pestaña, `Ctrl+Mayús+T` reabrir pestaña cerrada, `Ctrl+D` propiedades |
| Navegación | `Ctrl+Tab`/`Ctrl+Mayús+Tab` pestañas, `Ctrl+G` ir a página, `Inicio`/`Fin`, `RePág`/`AvPág`, `Alt+←/→` atrás/adelante en la historia de saltos |
| Zoom y vista | `Ctrl++`/`Ctrl+-`, `Ctrl+0` ajustar página, `Ctrl+1` 100 %, `Ctrl+2` ajustar ancho, `Ctrl+rueda`, `Ctrl+R`/`Ctrl+Mayús+R` rotar vista, `F11` presentación |
| Buscar | `Ctrl+K` paleta, `Ctrl+F` buscar en el documento, `F3`/`Mayús+F3` siguiente/anterior |
| Paneles | `F4` panel lateral, `F9` inspector, `F6`/`Mayús+F6` recorrer regiones, `Ctrl+Mayús+A` todas las herramientas |
| Modos | `Alt+1`…`Alt+9` según posición en la barra |
| Herramientas de modo | Una letra cuando el foco está en el lienzo (ver §4.2); `Esc` vuelve a Seleccionar |
| Edición | `Ctrl+Z`, `Ctrl+Y`, `Supr`, `Ctrl+C`, `Ctrl+A` (página/selección) |

Todos los atajos se pueden ver con `F1` (hoja de atajos) y cambiar en Ajustes > Avanzado > Atajos.

## 7. Accesibilidad
- Todo operable con teclado; orden de tabulación: barra de título → modos → barra del modo → raíl → panel → lienzo →
  inspector → barra de estado. `F6` recorre regiones como en las apps de Windows.
- Roles ARIA correctos (tablist, toolbar, tree para marcadores, grid para miniaturas). WebView2 los expone por UIA a
  Narrador y NVDA. Probar con Narrador en acr12.
- Contraste ≥ 4,5:1 en texto y ≥ 3:1 en iconos y bordes de controles (ver `sistema-visual.md`).
- Alto contraste de Windows (`forced-colors`), escala de texto de Windows (unidades relativas al tamaño base),
  movimiento reducido y modo de lectura nocturna (invierte el papel, no la interfaz).
- Lectura en voz alta del documento con las voces de Windows (SAPI/OneCore) desde el modo Leer.

## 8. Idiomas
Español (es-ES) e inglés (en) desde el principio; claves por área (`viewer.zoom.fitWidth`). Formatos con `Intl`:
"12,4 MB", "3 nov. 2026". Textos de la interfaz en el idioma de Windows salvo que se elija otro.

## 9. Vocabulario (glosario de interfaz)

| Usamos | No usamos | Motivo |
| --- | --- | --- |
| Organizar páginas | Organizar páginas *Pro*, Herramientas de página | Llano |
| Comprimir | Reducir tamaño de archivo | Más corto; "Optimizar avanzado" para lo técnico |
| Reconocer texto (OCR) | Mejorar escaneos | Describe lo que hace |
| Redactar | Censurar, Tachar datos | Término técnico extendido; tooltip "Eliminar de forma permanente texto e imágenes" |
| Firma digital | Certificados | Lo que la gente busca |
| Mi firma | Firmar uno mismo | Firma manuscrita/imagen, sin certificado |
| Pedir firmas | Solicitar firmas electrónicas | Local, sin nube |
| Inspector | Panel de propiedades | Una palabra; mismo sitio para propiedades y tareas |

## 10. Contrato para los flujos (cómo se integra una herramienta)

Cada herramienta se declara en el registro del frontend; la barra de modos, la paleta de herramientas, la paleta de
órdenes, los atajos y el menú contextual se generan a partir de él. Ningún flujo toca el armazón para añadir su
herramienta.

```ts
// app/src/tools/<area>/manifest.ts
import type { ToolManifest } from "@/tools/registry";

export const compress: ToolManifest = {
  id: "optimize.compress",          // único, "<area>.<herramienta>"
  group: "compare-optimize",        // grupo de la paleta (§4.3)
  kind: "task",                     // "mode" | "task" | "wizard" | "panel" | "screen"
  title: "tools.compress.title",    // clave i18n
  description: "tools.compress.description",
  icon: "file-down",                // nombre de icono (app/src/ui/icons)
  keywords: ["reducir", "tamaño", "optimizar", "compress"],
  shortcut: undefined,              // "Ctrl+Alt+C" si aplica
  when: { document: true },         // requisitos: documento abierto, tiene formularios, tiene firmas, etc.
  pinnable: true,
  load: () => import("./CompressPanel"), // componente perezoso: panel de tarea / barra del modo / pantalla
};
```

Backend: cada flujo añade `app/src-tauri/src/commands/<area>.rs` con sus comandos y los registra con una línea en
`commands/mod.rs`. Las operaciones largas devuelven un `JobId` y emiten `job-progress {job_id, done, total, label}` y
`job-finished {job_id, result | error}`; se cancelan con `cancel_job(job_id)`. Las que modifican el documento abierto
suben su revisión y emiten `document-changed`.

Componentes compartidos (en `app/src/ui/`): `Button`, `IconButton`, `ToggleGroup`, `Menu`, `Tooltip`, `Field`,
`Select`, `Slider`, `Checkbox`, `Radio`, `Tabs`, `Inspector`, `TaskPanel` (título, opciones, avanzado, pie con
acciones y progreso), `WizardSteps`, `Notice` (barra de aviso), `Toast`, `EmptyState`, `Kbd`. Ningún flujo crea
estilos propios para estos casos.
