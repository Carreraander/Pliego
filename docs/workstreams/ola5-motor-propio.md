# Ola 5: sustituir MuPDF por PDFium + lopdf detrás de `pliego-pdf` (W30-W41)

Decisión de Ander (2026-11-04 23:22, tarjeta del estudio): **opción B1**. Detalle técnico, medidas y riesgos en
[`docs/design/licencia-y-motor.md`](../design/licencia-y-motor.md) §2, §5, §7 y §8. Licencia candidata (pendiente de
revisión legal): BSL 1.1 con uso gratuito para empresas; paga quien venda, integre en un producto de pago u ofrezca
Pliego como servicio de pago. **`LICENSE`, `deny.toml`, `THIRD-PARTY.md`, cabeceras, web y CLA no se tocan hasta que
MuPDF haya salido del binario y haya revisión legal** (un único PR final, W41).

Esta decisión **sustituye** a «AGPL-3.0 + MuPDF» (2026-11-02) en cuanto se complete; mientras tanto el repo sigue
siendo AGPL.

## Principios

1. **Cada paso deja `main` verde y entregable.** Se implementa primero la capa sobre MuPDF; el motor se cambia después
   detrás de la capa, crate a crate, y siempre se puede revertir.
2. **Ningún crate habla con MuPDF, PDFium ni lopdf directamente** cuando termine la fase A. Solo `pliego-pdf` lo hace.
3. **Las pruebas actuales (706) son el contrato**: la migración es mecánica y no cambia comportamiento. Un cambio de
   comportamiento va en PR aparte.
4. **El cuello de botella es verificar** (un turno en acr12, revisión visual de Ander): el arnés de comparación
   (W34) empieza el primer día para que cada PR llegue con evidencia automática.
5. **La ola 4 sigue.** Los PRs de W24-W29 que ya usan `mupdf` directamente se migran después; gestión coordina el orden
   para que un crate no se migre mientras tenga un PR abierto (ver «Reglas de coordinación»).

## Fases y flujos

### Fase A: la capa `pliego-pdf` sobre MuPDF (bloque 1 del estudio, 6-8 semanas-hilo)

| Flujo | Tema | Depende de |
| --- | --- | --- |
| **W30** | Diseño y primera versión de `pliego-pdf`: tipos propios (`Document`, `Page`, `Object`, `Dict`, `TextPage`, `RenderOptions`, `SaveOptions`, errores), implementación sobre MuPDF, documentación de contrato en `docs/ARCHITECTURE.md` y reglas nuevas en `docs/CONVENTIONS.md`. Migra `pliego-core`, `engine-mupdf` (→ `pliego-engine`), `worker` y `cli` (lectura). **Congela la API en 2-3 días** y la publica en `docs/handoff/flujos/w30.md` | nada |
| **W31** | Migrar a la capa los crates de **modelo de objetos y guardado**: organize, portfolio, pdfa, a11y, protect, optimize, stamp, annot | API de W30 congelada |
| **W32** | Migrar los crates de **render y texto**: compare, print, measure, index, ocr, office-export, convert, redact (solo lectura de texto/geometría en esta fase) | API de W30 congelada |
| **W33** | Migrar **formularios, edición y app**: forms, formdesign, edit, `app/service`, `app/src-tauri` | API de W30 congelada |

W31-W33 trabajan en paralelo sobre crates distintos. Si un crate necesita una función que la capa no tiene, se pide a
W30 (PR pequeño que **solo añade**), nunca se importa `mupdf` de nuevo. Al terminar la fase A, un `grep` de `mupdf::`
fuera de `pliego-pdf` debe dar cero y una prueba de CI lo comprueba.

### En paralelo desde el primer día

| Flujo | Tema | Depende de |
| --- | --- | --- |
| **W34** | **Arnés de comparación y corpus** (bloque 10): para cada PDF de `tests/corpus` (85) y del corpus externo, render a 72 y 150 ppp con dos motores y diferencia de píxeles con umbral, texto extraído, salida validada con `qpdf --check` y veraPDF. Funciona con «MuPDF contra MuPDF» (línea base) y luego con el motor nuevo. Incluye el oráculo de redacción (texto no extraíble, cadenas en flujos sin comprimir) y fuzzing (`fuzz/`) | nada |
| **W35** | **Empaquetado de `pdfium.dll`** (bloque 9): prototipo con `pdfium-render` y `pdfium-binaries`, carga dinámica, instalador (`installer/setup`), actualizaciones, worker aislado, avisos de licencia (BSD-3/Apache-2.0), versión fijada y verificada por hash. Spike de memoria y arranque frente a los objetivos de W01 | nada |

### Fase B: el motor nuevo detrás de la capa (tras W30 y, por bloque, las migraciones que toque)

| Flujo | Tema (bloque) | Depende de | Riesgo |
| --- | --- | --- | --- |
| **W36** | Modelo de objetos, guardado completo e incremental con **lopdf** (bloque 2) | W30, W31 | Medio |
| **W37** | **Render, texto y búsqueda con PDFium** (bloque 3) | W30, W32, W35 | Medio |
| **W38** | **Redacción real sin MuPDF** (bloque 4): reescritura de flujos de contenido y recorte de imágenes; verificación obligatoria con el oráculo de W34 | W36, W37 | **Alto** (seguridad) |
| **W39** | Edición de texto, fuentes, sellos e imágenes (bloque 5) y **formularios y apariencias** (bloque 6) | W33, W36, W37 | Alto |
| **W40** | **Cifrado** (lectura RC4/AES-128/AES-256, escritura AES-256, bloque 7) y **conversión** (imágenes↔PDF, DOCX; XPS/EPUB/HTML se quitan o se convierten fuera, bloque 8) | W36, W37 | Medio |

### Cierre

| Flujo | Tema | Depende de |
| --- | --- | --- |
| **W41** | MuPDF fuera del binario: quitar la dependencia, `cargo deny` sin AGPL, y **en un solo PR** `LICENSE`, `deny.toml`, `THIRD-PARTY.md`, cabeceras, web y CLA (solo tras la revisión legal de Ander). Dos rondas de revisión manual de Ander | W36-W40, W34 en verde |

## Reglas de coordinación

- **Orden de migración sin pisar PRs:** gestión lleva una tabla (en `docs/handoff/flujos/w30.md`) con crate → estado
  (pendiente / en migración / migrado). Un crate «en migración» no recibe PRs de ola 4; si W24-W29 tiene uno abierto, se
  fusiona antes o la migración espera.
- **Código nuevo desde que W30 congele la API:** usa `pliego-pdf`; los PRs de ola 4 que aún importan `mupdf` se aceptan
  hasta ese día y después se migran en W31-W33.
- **Cada PR de migración** debe mostrar las pruebas del crate en verde sin cambios, `cargo clippy`, y el arnés (W34) sin
  regresiones. Sin cambios de comportamiento ni de formato de salida.
- **Backends:** cada PR de fase B cambia una función de la capa de MuPDF al motor nuevo y mantiene la implementación
  MuPDF disponible detrás de una característica de Cargo hasta W41, para poder comparar y revertir.
- **Calidad del motor:** si la diferencia de píxeles del arnés supera el umbral en más de lo acordado en `known-issues`,
  el PR no se fusiona; se abre incidencia con el PDF.
- **acr12:** un turno por flujo; el empaquetado y el rendimiento se miden allí (apertura, render, índice de 1.000 PDF
  en ~3 s, memoria 150 MB app+worker). Mismas reglas de siempre (`acr12-turno.ps1`).
- **Hilos:** Sonnet 5.5 por flujo, salvo W30 y W38 (diseño de la capa y redacción), que conviene llevar con Opus 5.5.

## Criterio de hecho de la ola

- `mupdf` no aparece en ningún `Cargo.toml` ni en el binario; `cargo deny check licenses` pasa sin AGPL.
- Las 706 pruebas siguen en verde y el arnés no muestra regresiones frente al corpus; fuzzing ≥ 24 h sin caídas nuevas.
- La redacción pasa el oráculo (ni texto extraíble ni cadenas en los flujos bajo el área; imágenes recortadas).
- Instalador, autoactualización y rendimiento medidos en acr12; revisión manual de Ander en dos rondas.
- Licencia nueva publicada en un único PR, con revisión legal hecha.
