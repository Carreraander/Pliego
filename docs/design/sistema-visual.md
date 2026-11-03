# Pliego · Sistema visual

Estado: propuesta v1 (2026-11-03). Tokens: `docs/design/tokens.css` (fuente de verdad, prefijo `--pl-`).

## Dirección: "grafito y tinta"
Un *pliego* es la hoja grande que se dobla para formar un cuadernillo. La identidad toma de la imprenta tres cosas:
el doblez (logo, pestaña activa), la tinta (acento verde azulado, el color de la tinta de impresión) y las marcas de
registro (indicador de carga). El cromo es grafito neutro con un sesgo verde muy leve hacia el acento, para que el
papel blanco de la página sea lo más luminoso de la pantalla y los colores del documento no compitan con la interfaz.

Se descarta el crema cálido del boceto del PR #1 (#f4f1ea): amarillea el papel blanco por contraste y es un aspecto
muy repetido. Se mantiene su idea de acento verde tinta, más saturado y oscuro para cumplir contraste.

Nada de rojo como color de marca (asociado a Adobe). El rojo es solo semántico (error, redacción).

## Color

| Token | Claro | Oscuro | Uso |
| --- | --- | --- | --- |
| `--pl-chrome` | `#f2f3f1` | `#1b1d1c` | Barra de título (bajo Mica), barras de herramientas |
| `--pl-surface` | `#fbfbfa` | `#222524` | Paneles, inspector, menús |
| `--pl-surface-raised` | `#ffffff` | `#2a2d2c` | Diálogos, popovers, tarjetas |
| `--pl-surface-sunken` | `#e8eae6` | `#161817` | Campos de texto, pistas |
| `--pl-canvas` | `#dfe2de` | `#121413` | Fondo del lienzo |
| `--pl-text` / `-2` / `-3` | `#1b1f1d` / `#545b57` / `#7d847f` | `#eceeec` / `#a8afab` / `#7a817d` | Texto principal / secundario / solo no esencial |
| `--pl-accent` | `#0f6b62` | `#4cc2b2` | Botón primario, modo activo, foco, enlaces |
| `--pl-accent-soft` | `#d7ebe7` | `#173a36` | Fondo de herramienta activa y fila seleccionada |
| Semánticos | ok `#23733f`, aviso `#8a5300`, error `#b42318`, info `#255f9e` | versiones claras | Estado de firma, avisos, errores |

Reglas:
- El acento marca **lo activo o lo principal**, una cosa por zona: el modo actual, la herramienta activa, el botón
  primario del panel. Si todo es acento, nada lo es.
- Contraste comprobado: texto ≥ 4,5:1 sobre su fondo; `--pl-text-3` (3,6:1) solo en texto no esencial.
  Iconos y bordes de controles ≥ 3:1.
- El papel de la página es siempre blanco, también en tema oscuro (los documentos son blancos). El modo de lectura
  nocturna invierte el render de la página, no la interfaz.
- La paleta de anotaciones (`--pl-annot-*`) no cambia con el tema: es lo que se escribe en el PDF.
- Mica: la barra de título usa `windowEffects: ["mica"]` de Tauri con `--pl-chrome` al 80 % de opacidad encima;
  en Windows 10 (sin Mica) queda `--pl-chrome` opaco.
- Opción en Ajustes: "Usar el color de énfasis de Windows" (desactivada por defecto). Si se activa, se recalcula
  `--pl-accent*` desde el color del sistema garantizando 4,5:1.

## Tipografía
Fuente del sistema: **Segoe UI Variable** (Windows 11; en Windows 10, Segoe UI). Se integra con el resto de Windows,
no añade peso al instalador y renderiza con ClearType/DirectWrite. La personalidad la ponen el doblez, el acento y la
composición, no una fuente de marca.

| Rol | Tamaño / interlineado | Peso | Uso |
| --- | --- | --- | --- |
| Caption | 12 / 16 | 400 | Barra de estado, metadatos, atajos |
| Body | 14 / 20 | 400 | Todo el texto de la interfaz |
| Body strong | 14 / 20 | 600 | Nombres de pestaña activa, etiquetas de grupo |
| Subtitle | 16 / 22 | 600 | Título del inspector y de paneles |
| Title | 20 / 28 | 600 | Títulos de Ajustes, pantalla de inicio |
| Display | 28 / 36 | 600 (Display) | Saludo de inicio, instalador |

Números en columnas (tamaños, páginas, porcentajes): `font-variant-numeric: tabular-nums`. Mayúsculas solo en
etiquetas de grupo pequeñas, con `letter-spacing: 0.04em`.

## Espaciado, radios y elevación
- Rejilla de 4 px (`--pl-space-1` = 2 px … `--pl-space-10` = 48 px). Espaciado interno de paneles: 16 px; entre
  controles relacionados: 8 px; entre grupos: 16–24 px.
- Radios por función: 4 px chips y miniaturas, 6 px controles, 8 px menús y pestañas, 12 px diálogos y ventana del
  instalador. Las páginas del documento **no** tienen radio (son papel).
- Elevación con tres niveles de sombra; los paneles acoplados no tienen sombra, solo borde.

## Densidad
Tres densidades (Ajustes > General): compacta (controles de 28 px), cómoda (32 px, por defecto) y táctil (40 px).
Si Windows está en modo tableta o hay pantalla táctil sin teclado, se sugiere táctil una vez.

## Iconos
- Base: **Lucide** (ISC), trazo 1,75 px a 20 px (16 px en barras densas), esquinas redondeadas.
- Iconos propios de PDF que Lucide no tiene (redactar, Bates, PDF/A, firma PAdES, OCR, separaciones, sello de tiempo,
  doblez del logo) en `app/src/ui/icons/custom/`, dibujados en la misma rejilla 24×24 y trazo.
- Un icono nunca va solo sin nombre accesible; en la barra del modo, tooltip con nombre y atajo.

## Logo
Hoja cuadrada en el color de acento con la esquina superior derecha doblada hacia delante (triángulo más claro) y el
pliegue marcado en diagonal. Lectura: una hoja doblada, es decir, un pliego. Variantes: color sobre claro, color sobre
oscuro (`--pl-accent` oscuro), monocromo para la barra de tareas en alto contraste. SVG en el prototipo; el icono
`.ico` (16, 20, 24, 32, 48, 64, 256 px) se genera a partir del SVG con ajuste manual a 16 px.

## Detalles propios
- **Pestaña activa**: la esquina superior derecha de la pestaña dibuja un pequeño doblez (6 px).
- **Carga**: una marca de registro de imprenta (círculo con cruz) que gira; en el instalador, la hoja que se dobla.
- **Barra de estado**: muestra el tamaño de papel en su nombre normalizado y en mm ("A4 · 210 × 297 mm";
  "Carta · 216 × 279 mm"; tamaños no estándar solo en mm).
- **Zoom**: valores con los nombres de imprenta en el desplegable: "Ajustar página", "Ajustar ancho", "Tamaño real".

## Movimiento
- 100 ms para estados de hover/pulsado, 180 ms para paneles que se abren, 280 ms para cambios de pantalla.
- Curva estándar `cubic-bezier(0.2, 0, 0, 1)`. Nada rebota.
- `prefers-reduced-motion`: duraciones a 0; el zoom y el desplazamiento siguen siendo inmediatos.
- El lienzo nunca anima el contenido de la página: el render progresivo (baja resolución → teselas nítidas) es la
  única transición que se ve sobre el papel.
