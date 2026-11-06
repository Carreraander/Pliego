# Licencia propia y motor PDF: estudio de decisión

Versión 1 (2026-11-04), hilo «Estudiar sustituir MuPDF y licencia propia». Documento de decisión: **no cambia código,
licencias ni dependencias**. Las decisiones son de Ander.

> **Esto no es asesoramiento legal.** Resume cómo funcionan las licencias citadas según sus textos públicos y la
> práctica habitual. Antes de cambiar la licencia o firmar con Artifex, que lo revise un abogado de propiedad
> intelectual (en España o la UE).

## 1. Qué hay que decidir

> **Decidido (Ander, 2026-11-04 23:20):** opción (a): uso gratis para todos, empresas incluidas; paga solo quien
> venda Pliego o derivados, lo integre en un producto de pago o lo ofrezca como servicio de pago. Licencia candidata:
> BSL 1.1.
>
> **Decidido (Ander, 2026-11-04 23:22):** motor **B1**, sustituir MuPDF por PDFium + lopdf detrás de la capa
> `pliego-pdf`, por las fases de §7. La licencia no cambia hasta que MuPDF salga del binario; antes, revisión legal.

1. **Modelo de licencia.** Lo que pide Ander («cualquiera lo usa y ve el código, pero quien lo comercialice me paga una
   comisión») no es *open source* según la OSI: es **source-available** (código visible). Hay que elegir qué cuenta
   como «comercializar»:
   - (a) solo **vender o redistribuir** Pliego o derivados, o venderlo como servicio, y el uso por empresas es gratis; o
   - (b) **cualquier uso comercial**, incluido que una empresa lo use internamente.
2. **Motor PDF.** Con cualquier licencia que no sea AGPL, MuPDF (AGPL) obliga a comprar licencia comercial a Artifex
   (opción A) o a sustituirlo (opción B o C).
3. **Revocar la decisión del 2026-11-02** («AGPL-3.0 + MuPDF»), que está en `docs/handoff/README.md` como no reabrir
   sin Ander.

**Recomendación (detalle en §7):** opción **B** (PDFium + lopdf detrás de una capa de motor propia) con licencia
**Business Source License 1.1** si Ander elige (a), o **PolyForm Noncommercial 1.0.0** si elige (b); registrar la
marca «Pliego»; y no cambiar la licencia hasta que MuPDF haya salido del binario.

## 2. Cuánto depende Pliego de MuPDF (medido en `main` @ 16831e2)

| Medida | Valor |
| --- | --- |
| Código Rust del workspace | ~69.400 líneas, 706 pruebas `#[test]` |
| Crates que usan el crate `mupdf` directamente | **22** de 37 (+ `app/service`); 4 más vía `pliego-engine-mupdf` (cli, worker, qa, index) |
| Archivos `.rs` que importan `mupdf` | **128**, que suman ~27.700 líneas (≈40 % del código) |
| Usos de `PdfDocument` / `PdfObject` (modelo de objetos de MuPDF) | ~620 llamadas a objetos (`get_dict`, `dict_put`, `resolve`…) |
| Crates sin MuPDF | `pliego-sign` (firma PAdES con **lopdf**, MIT), `update`, `release`, `setup`, `install`, `launcher`, `appdata`, `actions`, `assistant`, `signflow`, `steps`, `core` |

El trait `PdfEngine`/`EngineDocument` de `pliego-core` solo cubre lectura (info, render, texto, búsqueda, marcadores).
Todo lo demás llama a tipos de MuPDF directamente; la propia regla de `docs/CONVENTIONS.md` («funciones puras sobre
`mupdf::pdf::PdfDocument`») acopla las bibliotecas a MuPDF. **Por eso el cambio de motor es grande: no hay una costura
única donde cortar.**

Qué hace MuPDF hoy, por área (crates afectados):

| Área | Funciones de MuPDF usadas | Crates | Dificultad de sustituir |
| --- | --- | --- | --- |
| Render y teselas | `to_pixmap`, `Matrix`, `Colorspace`, `Device` | engine-mupdf, worker, compare, print, formdesign, measure, ocr | Media (PDFium lo hace bien) |
| Texto, búsqueda, orden | `to_text_page`, `StructuredText`, `Quad` | engine, index, redact, convert, office-export, a11y, compare, edit | Media |
| Modelo de objetos y guardado | `PdfDocument`, `PdfObject`, `save_with_options` (incremental, limpieza, compresión), `graft_object` | organize, portfolio, pdfa, a11y, protect, optimize, stamp, forms… | Alta por volumen, técnica baja (lopdf) |
| Cifrado | `authenticate`, `PdfWriteOptions` (cifrar/descifrar, permisos) | protect y casi todos al abrir | Media (lopdf cifra AES-256; verificar AES-128/RC4) |
| Redacción | `redact_with_options` (quita texto, imágenes y vectores bajo el área) | redact, edit | **Alta**: PDFium no aplica redacciones; hay que reescribir flujos de contenido. Es una función de seguridad |
| Edición de texto y fuentes | `Font`, `add_simple_font`, `PathWalker`, `insert_image`, `run_contents` | edit, stamp, annot, optimize | Alta |
| Formularios y anotaciones | `PdfWidget`, `create_annotation`, apariencias, `bake` (aplanar) | forms, formdesign, annot | Media-alta (PDFium genera apariencias y aplana; la API difiere) |
| Conversión | entrada XPS/EPUB/CBZ/HTML/SVG, salida DOCX, `DocumentWriter` | convert, office-export | Media; XPS/EPUB/HTML de entrada se pierden o se sustituyen |
| Fuentes del sistema | `set_font_loader`, `init_fonts` | engine-mupdf | Baja |

El crate `mupdf` (bindings de Rust, de messense) también es **AGPL-3.0**, no solo la biblioteca C de Artifex.

## 3. Licencias: lo que pide Ander y lo que existe

**Por qué no es open source.** La definición de la OSI (criterio 6) prohíbe discriminar campos de uso: una licencia
open source no puede impedir el uso comercial. La AGPL actual **sí permite** que cualquiera venda Pliego; solo obliga a
publicar el código (también de las modificaciones y del uso como servicio). Por eso la AGPL protege contra versiones
cerradas, pero **no** garantiza que nadie lo comercialice sin pagar.

| Licencia | Qué permite gratis | Qué exige pago | ¿Open source? | Notas |
| --- | --- | --- | --- | --- |
| **AGPL-3.0 + licencia comercial** (doble licencia, lo de ahora ampliado) | Todo, incluido vender, si se publica el código | Solo quien quiera cerrar el código | Sí | No impide comercializar. Con MuPDF, Ander necesitaría licencia de redistribución de Artifex para vender excepciones |
| **PolyForm Noncommercial 1.0.0** | Uso personal, educativo, ONG, administración pública | Todo uso comercial, **incluido el uso interno en empresas** | No | Texto estándar, corto y claro. Encaja con (b) |
| **PolyForm Small Business 1.0.0** | Todo, a empresas de < 100 personas y < 1 M USD de ingresos | Empresas grandes | No | Variante intermedia |
| **PolyForm Shield 1.0.0** | Todo salvo competir con el autor | Productos que compitan | No | No cubre «vender Pliego» si no compite… ambiguo para este caso |
| **Business Source License 1.1 (BSL)** | Lo que diga el «Additional Use Grant» que redacte Ander | El resto | No, pero cada versión pasa a una licencia open source (p. ej. Apache-2.0 o AGPL) en una «Change Date» de ≤ 4 años | Plantilla parametrizable (MariaDB, antes HashiCorp). Encaja con (a) |
| **Functional Source License (Fair Source)** | Todo salvo usos que compitan comercialmente | Uso competidor | No; pasa a Apache/MIT a los 2 años | Pensada para SaaS; poco precisa para «revender» un programa de escritorio |
| **Elastic License 2.0** | Casi todo | Ofrecerlo como servicio gestionado, saltarse claves de licencia | No | Pensada para servicios en la nube; no impide vender copias |
| **Commons Clause** (añadido a otra licencia) | Lo que permita la base | «Vender» el software | No | Redacción ambigua y criticada; **no** se puede añadir a la AGPL/GPL (prohíben restricciones adicionales) |

La «comisión X» no va en la licencia: la licencia dice «para este uso necesitas una licencia comercial» y el contrato
comercial (que Ander redacta con un abogado) fija el precio, la regalía o el porcentaje.

**Consecuencias de pasar a source-available:** no se puede llamar «open source» ni «software libre» (la landing de
pdfpliego.com lo tendría que corregir); algunas distribuciones y tiendas (winget sí lo acepta; Debian/F-Droid no) y
contribuidores lo rechazan; la confianza se apoya en poder auditar el código, que se mantiene.

### Licencias del resto de dependencias

Todas son compatibles con una licencia source-available si se cumplen sus avisos (copiar textos de licencia y
NOTICE en `licenses\`, como ya hace el instalador):

| Dependencia | Licencia | Conclusión |
| --- | --- | --- |
| MuPDF (C) y crate `mupdf`/`mupdf-sys` | AGPL-3.0 (o comercial de Artifex para la parte C) | **Bloquea** cualquier licencia no AGPL |
| Tesseract 5, tessdata | Apache-2.0 | OK (incluir NOTICE) |
| Leptonica y DLL de vcpkg (libpng, zlib, libtiff, libjpeg, libwebp, openjpeg, giflib) | BSD-2 / zlib / libtiff / IJG / MIT | OK |
| lopdf, rustybuzz, ttf-parser, subsetter, RustCrypto (rsa, p256…), cms, x509-cert | MIT / Apache-2.0 | OK |
| ring, webpki-roots | ISC/MIT/OpenSSL; CDLA-Permissive-2.0 | OK |
| Crates MPL-2.0 (permitidos en `deny.toml`) | MPL-2.0 (copyleft por archivo) | OK: solo obliga a publicar cambios a esos archivos |
| Tauri, wry, SolidJS, Lucide, Vite | MIT / Apache-2.0 / ISC | OK |
| WebView2 Runtime | Licencia de redistribución de Microsoft | OK (ya se usa así) |
| Noto Sans CJK (descarga opcional) | SIL OFL-1.1 | OK |
| PDFium (si se elige B) y sus terceros (FreeType, libjpeg-turbo, OpenJPEG, lcms2, zlib, libpng, agg, ICU) | BSD-3 / Apache-2.0 / FTL / IJG / MIT | OK; no compilar XFA/V8 |
| `pdfium-render` (bindings Rust), binarios de `bblanchon/pdfium-binaries` | MIT o Apache-2.0 | OK |
| hayro (alternativa B2) | MIT o Apache-2.0 | OK |

`deny.toml` habría que actualizarlo (quitar AGPL de `allow` y añadir la licencia elegida para los crates propios).

### ¿Hace falta un CLA para relicenciar?

- **Código ya escrito:** `git log` muestra un único autor (`Carreraander`): todo lo han escrito agentes por cuenta de
  Ander, sin contribuciones de terceros. Ander puede relicenciarlo él solo; **no hace falta CLA para el pasado**.
  Comprobar antes de cambiar que ningún PR externo se haya fusionado entretanto.
- **Lo ya publicado sigue siendo AGPL.** El repositorio es público desde el 2026-11-02. Cualquiera que tenga una copia
  de un commit anterior al cambio puede seguir usándola, modificándola y **vendiéndola** bajo AGPL para siempre. El
  cambio solo afecta al código nuevo. Cuanto antes se decida, menos valor tiene esa versión AGPL (y sin el motor nuevo
  sigue atada a MuPDF AGPL).
- **Contribuciones futuras:** con licencia propia y ventas, hace falta un **CLA** que ceda o licencie a Ander los
  derechos con permiso para relicenciar y sublicenciar comercialmente (un DCO no basta). Plantillas: Harmony
  Agreements, el CLA de Apache adaptado, o CLA Assistant en GitHub.
- **Código generado por IA:** en la UE y en España la protección por derechos de autor exige autoría humana. Si casi
  todo el código lo escriben agentes, la base para hacer cumplir **cualquier** licencia (también la AGPL) es más débil.
  Ayuda documentar las decisiones humanas (diseño, revisión, selección). Es la primera pregunta para el abogado.
- **La marca protege más que la licencia.** Registrar «Pliego» (OEPM o EUIPO, clases 9 y 42) impide que una copia se
  venda con ese nombre, sea cual sea la licencia del código.

## 4. Opción A: licencia comercial de Artifex + doble licencia

**Qué es.** Ander compra a Artifex una licencia comercial de MuPDF que le permita distribuir Pliego sin AGPL. El
código de Pliego pasa a la licencia propia (o AGPL + comercial).

| | |
| --- | --- |
| Esfuerzo de código | 1-2 semanas-hilo (licencias, avisos, `deny.toml`, CLA, textos de la web) |
| Esfuerzo extra | Los bindings `mupdf`/`mupdf-sys` son AGPL de terceros (messense y colaboradores): pedirles permiso por escrito o escribir bindings propios sobre la API C (3-5 semanas-hilo, solo los ~50 tipos que usa Pliego) |
| Coste | **Desconocido: Artifex cotiza caso a caso y no publica precios.** Suelen ser cuotas anuales o regalías por unidad; pedir presupuesto antes de nada |
| Pruebas | Las actuales bastan (CI, corpus nocturno, acr12). Revisión legal del contrato |
| Riesgos | Coste recurrente y dependencia de Artifex (renovaciones, condiciones de auditoría). Hay que confirmar que Artifex admite publicar el código de Pliego como source-available: quien compile desde el código obtiene MuPDF bajo AGPL y no podría redistribuir la mezcla, solo usarla. Y si Ander vende licencias comerciales a terceros que integren Pliego, cada uno necesitaría también licencia de Artifex o una licencia OEM de Ander |
| Calendario | Lo marca la negociación (semanas) |

## 5. Opción B: sustituir MuPDF por motores permisivos

**Propuesta B1 (recomendada): PDFium + lopdf + capa propia.**

- **Capa `pliego-pdf`** (nueva): tipos propios `Document`, `Page`, `Object`, `TextPage`, `RenderOptions`, guardado.
  Todos los crates hablan con ella; ninguno ve PDFium ni lopdf. Primero se implementa **sobre MuPDF**, se migran los
  crates con las pruebas actuales en verde, y luego se cambia el motor detrás. Así cada paso se puede probar y
  revertir.
- **PDFium** (Google, BSD-3/Apache-2.0, vía `pdfium-render` y binarios de `pdfium-binaries`): render, texto y
  búsqueda, formularios (apariencias y aplanado), anotaciones, imágenes.
- **lopdf** (MIT, ya en `pliego-sign`): modelo de objetos, organizar páginas, adjuntos, metadatos, PDF/A,
  etiquetas, guardado completo e incremental, cifrado AES-256. qpdf (Apache-2.0) es un plan B para linealizar o reparar.
- **Propio:** redacción (reescribir flujos de contenido y recortar imágenes), edición de texto (ya usa rustybuzz y
  subsetter), conversiones.
- **Se pierde:** abrir XPS, EPUB, CBZ, HTML y SVG como documento (MuPDF los trataba como PDF). CBZ e imágenes se
  rehacen fácil; XPS/EPUB se quitan o se convierten con otra herramienta.

| Bloque | Semanas-hilo | Riesgo |
| --- | --- | --- |
| 1. Capa `pliego-pdf` sobre MuPDF y migrar los 22 crates a ella (cambio mecánico, mismas pruebas) | 6-8 | Bajo |
| 2. Modelo de objetos, guardado e incremental con lopdf (organize, portfolio, pdfa, a11y, protect, optimize, stamp…) | 5-7 | Medio |
| 3. Render, texto y búsqueda con PDFium (engine, worker, index, compare, print, ocr, measure, office-export) | 4-5 | Medio |
| 4. Redacción real sin MuPDF | 4-6 | **Alto** (si queda texto recuperable es un fallo de seguridad) |
| 5. Edición de texto, fuentes, sellos e imágenes | 3-5 | Alto |
| 6. Formularios y apariencias de anotaciones, aplanar | 3-4 | Medio |
| 7. Cifrado (lectura RC4/AES-128/AES-256, escritura AES-256) | 1-2 | Medio |
| 8. Conversión (imágenes↔PDF, DOCX, quitar o sustituir XPS/EPUB/HTML) | 2-3 | Medio |
| 9. Empaquetado de `pdfium.dll` (instalador, actualizaciones, worker aislado, avisos de licencia) | 1-2 | Bajo |
| 10. Arnés de comparación MuPDF ↔ nuevo motor y corpus | 3-4 | — |
| **Total** | **32-46 semanas-hilo** | Medio-alto |

**Calendario.** Con 6-7 hilos en paralelo: **6-10 semanas** de calendario, más 1-2 de pruebas en acr12 y revisión de
Ander. El bloque 1 va primero y casi todo lo demás en paralelo. Referencia: el proyecto escribió ~69.000 líneas en
unos tres días con unos 25 hilos, así que escribir el código puede ir más rápido que esto; **el cuello de botella es
verificar** (un solo turno en acr12 y la revisión visual de Ander). Un equipo humano equivalente serían 8-12
meses-persona.

**Variante B2: Rust puro (lopdf + hayro).** hayro (Laurenz Stampfl, MIT/Apache-2.0) es el rasterizador PDF en Rust
puro más completo, pero joven: ha tenido huecos en archivos cifrados y modos de fusión, y no trae extracción de texto
ni formularios. Evita una DLL C++ y encaja con `unsafe_code = "deny"`, pero añade 10-20 semanas-hilo (texto,
formularios) y más riesgo de fidelidad. Con la capa `pliego-pdf`, pasar de PDFium a hayro más adelante es cambiar solo
el render.

## 6. Opción C: motor propio desde cero

«Desde cero» significa escribir lo que MuPDF y PDFium llevan décadas puliendo (MuPDF desde 2004, PDFium de Foxit y
luego Google):

| Componente | Semanas-hilo |
| --- | --- |
| Analizador robusto y reparación de archivos rotos | 8-12 |
| Escritor, guardado incremental, cifrado | 4-6 |
| Intérprete de contenido y rasterizador (trazos, recortes, 7 tipos de sombreado, transparencia, grupos, máscaras, fusión) | 20-30 |
| Fuentes Type1, CFF, TrueType, Type3, CID, codificaciones | 12-20 |
| Decodificadores de imagen (DCT, JPX, JBIG2, CCITT, LZW…) | 8-12 (menos reutilizando crates) |
| Color (ICC, CMYK, Separation, DeviceN) | 6-10 |
| Texto estructurado, búsqueda, orden de lectura | 6-10 |
| Formularios, anotaciones, apariencias | 14-20 |
| Redacción | 4-6 |
| Robustez frente a PDFs maliciosos (fuzzing), rendimiento, memoria | 14-22 |
| **Total** | **~100-150 semanas-hilo** |

**Coste realista:** 6-12 meses de calendario con muchos hilos, y aun así fidelidad inferior a MuPDF o PDFium en PDFs
reales durante años (la cola larga de archivos raros es lo caro). Equivale a 3-6 personas-año para un motor «usable» y
bastantes más para igualar a MuPDF. Mayor superficie de ataque al abrir PDFs no confiables. **No compensa:** B ya da
un motor permisivo; lo único que C aporta es control total, que B2 (Rust puro con crates permisivos) da en gran parte.

## 7. Comparación y recomendación

| | A: Artifex | B1: PDFium + lopdf | B2: Rust puro | C: propio |
| --- | --- | --- | --- | --- |
| Esfuerzo | 1-2 (+3-5 bindings) semanas-hilo | 32-46 semanas-hilo | 45-65 semanas-hilo | 100-150 semanas-hilo |
| Calendario | Negociación | 6-10 semanas + pruebas | 3-4 meses | 6-12 meses |
| Coste externo | Licencia anual o regalías (por cotizar) | 0 | 0 | 0 |
| Calidad del motor | Igual que hoy | Muy buena (motor de Chrome) | Buena y mejorando | Inferior durante años |
| Riesgo | Contractual y económico | Medio-alto (redacción, edición de texto) | Alto (fidelidad) | Muy alto |
| Independencia | Depende de Artifex | Total | Total | Total |

**Recomendación: B1**, por fases y sin cambiar la licencia hasta el final:

1. Ander decide el modelo de licencia (a o b) y pide consejo legal (IA y derechos de autor, contrato comercial).
2. Pedir presupuesto a Artifex en paralelo: si es asumible, A es el camino corto mientras se hace B.
3. Fase 1: capa `pliego-pdf` sobre MuPDF (útil pase lo que pase; aislar el motor también facilita el proceso
   aislado pendiente de W02).
4. Fases 2-3: motor nuevo detrás de la capa, crate a crate, con el arnés de comparación.
5. Cuando MuPDF salga del binario: cambiar `LICENSE`, `deny.toml`, `THIRD-PARTY.md`, cabeceras, web y CLA en un solo
   PR, y publicar esa versión como la primera con la licencia nueva.

**Licencia recomendada:** si Ander quiere (a) «uso gratis para todos, pagar solo por vender o integrar», **BSL 1.1**
con un *Additional Use Grant* que permita todo uso, incluido el interno en empresas, salvo vender Pliego o derivados,
incluirlo en un producto de pago u ofrecerlo como servicio de pago; *Change Date* a 4 años hacia Apache-2.0 o AGPL.
Si quiere (b) «las empresas también pagan», **PolyForm Noncommercial 1.0.0** (o Small Business si las pequeñas
quedan gratis). Usar plantillas estándar, no una licencia redactada desde cero.

## 8. Pruebas necesarias para B1

- **CI (cada PR):** las 706 pruebas actuales siguen en verde en cada paso; la capa nueva con pruebas propias;
  `cargo deny` sin AGPL al final.
- **Arnés de comparación (nuevo, bloque 10):** para cada PDF del corpus, render a 72 y 150 ppp con los dos motores y
  diferencia de píxeles con umbral; texto extraído comparado; salida de cada herramienta validada con `qpdf --check` y,
  en PDF/A, veraPDF.
- **Corpus:** el corpus propio (`tests/corpus`, 85 PDF) y el externo de la CI nocturna (pdf.js, veraPDF, Isartor) con
  `corpus-runner`; sin caídas nuevas frente a `known-issues.txt`. Fuzzing del motor nuevo (`fuzz/`) al menos 24 h.
- **Redacción:** tras redactar, ni texto extraíble ni cadenas en los flujos sin comprimir bajo el área; imágenes
  recortadas; prueba con PDFs reales.
- **acr12:** empaquetado de `pdfium.dll`, instalador y autoactualización de extremo a extremo; rendimiento frente a
  hoy (apertura, render, índice de 1.000 PDF en ~3 s) y memoria (objetivo de W01: 150 MB app+worker, 500 MB con
  WebView2); firma con CNG y DNIe cuando haya lector.
- **Revisión manual de Ander:** dos rondas de 2-3 horas con la lista de `revision-manual-ander.md` (seis modos,
  abrir en Reader y en Acrobat lo que guarda Pliego, redactar, rellenar, firmar, imprimir).

## Fuentes

- Crate `mupdf` 0.8.0, licencia AGPL-3.0: <https://crates.io/crates/mupdf>
- Definición de open source de la OSI: <https://opensource.org/osd>
- PolyForm: <https://polyformproject.org/licenses/>
- Business Source License 1.1: <https://mariadb.com/bsl11/>
- Fair Source / FSL: <https://fair.io/>
- hayro: <https://docs.rs/crate/hayro/0.7.0/source/README.md>
- lopdf, ejemplo de cifrado AES-256: <https://docs.rs/crate/lopdf/latest/source/examples/encrypt.rs>
- PDFium, opciones de guardado: <https://android.googlesource.com/platform/external/pdfium/+/269ce88/public/fpdf_save.h>
- MuPDF, licencias: <https://artifex.com/licensing>
