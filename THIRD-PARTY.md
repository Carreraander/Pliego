# Componentes de terceros

Pliego se distribuye bajo la [Business Source License 1.1](LICENSE) (BUSL-1.1) con la concesión de uso adicional y la
licencia de cambio (AGPL-3.0-or-later) que fija ese archivo. Los componentes de terceros que se enumeran aquí conservan
sus propias licencias, que **no** cambian por estar dentro de Pliego. Todas son permisivas, salvo las cinco crates con
MPL-2.0 (copyleft por archivo), que se tratan aparte. **No hay ninguna dependencia GPL, LGPL ni AGPL** y `deny.toml`
impide que entre una (también prohíbe `mupdf` y `mupdf-sys`).

Este archivo recoge los componentes que **requieren mención expresa**. El listado completo de crates de Rust que se
distribuyen está en [`docs/legal/dependencias-rust.md`](docs/legal/dependencias-rust.md), y el estado legal del
proyecto, en [`docs/legal/resumen-para-abogado.md`](docs/legal/resumen-para-abogado.md).

El instalador copia este archivo y los textos de licencia de cada componente en `licenses\`, dentro de la carpeta de la
versión.

## PDFium (motor de dibujo, texto y búsqueda)

| | |
| --- | --- |
| Qué es | Biblioteca PDF de Google (Chromium), `pdfium.dll` |
| Versión | `chromium/8138`, binarios de [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries); versión, SHA-256 del paquete y de la DLL fijados en `tools/pdfium/pdfium.lock.json` |
| Licencia | BSD-3-Clause y Apache-2.0 (PDFium) |
| Cómo se usa | Se carga en tiempo de ejecución desde `pdfium.dll` (`PLIEGO_PDFIUM_DLL` o junto al `.exe`), sin enlazarla estáticamente. Sin XFA ni V8 |
| Crate de enlace | `pdfium-render` 0.9.5 (MIT o Apache-2.0) |
| Avisos | El instalador deja `LICENSE.pdfium` y la carpeta `licenses\` del paquete en `licenses\pdfium\`, junto a la DLL |

La DLL lleva a su vez código de terceros con sus propios avisos, que van en `licenses\pdfium\` y que hay que conservar
(entre otros: FreeType bajo FTL, libjpeg-turbo bajo IJG y BSD-3-Clause, OpenJPEG bajo BSD-2-Clause, lcms2 bajo MIT, zlib,
libpng, Abseil bajo Apache-2.0 e ICU bajo Unicode). La **FTL de FreeType exige** mencionar en la documentación:

> Portions of this software are copyright © The FreeType Project (www.freetype.org). All rights reserved.

La lista exacta de cada versión es la que acompaña al paquete fijado en `pdfium.lock.json`; al cambiar de versión hay
que revisarla.

## lopdf (modelo de objetos y guardado de PDF)

`lopdf` 0.46.0 (MIT). Lectura, escritura, guardado incremental y cifrado de la estructura de los PDF. Lo usan
`pliego-pdf` (capa `native`) y `pliego-sign`. Dependencias directas de lopdf: `nom` (MIT), `weezl`, `flate2`, `md-5`, `sha2`, `aes`, `cbc`, `ecb`, `rand`, `indexmap`,
`log`, `itoa`, `thiserror`, `bitflags`, `getrandom` (MIT o Apache-2.0), `rangemap` y `stringprep` (MIT/Apache-2.0),
`brotli-decompressor` (BSD-3-Clause/MIT) y `encoding_rs` ((Apache-2.0 o MIT) y BSD-3-Clause: la parte BSD-3-Clause son los
datos derivados de la especificación WHATWG y exige conservar su aviso, que va en `licenses\`).

## Boa 0.22 (motor de JavaScript de los formularios)

`crates/pliego-formjs` ejecuta los scripts de los formularios PDF con el intérprete [Boa](https://boajs.dev) 0.22.0
(`boa_engine`, `boa_ast`, `boa_gc`, `boa_interner`, `boa_macros`, `boa_parser`, `boa_string`), **MIT o Unlicense**; Pliego lo usa
bajo la MIT. Va compilado dentro de los tres ejecutables (`pliego-app`, `pliego`, `pliego-worker`), sin Temporal ni Intl.
Pide Rust 1.91 (`rust-version` del crate). El modelo de objetos de Acrobat (`prelude.js`) es propio, escrito a partir de la
documentación pública.

Dependencias transitivas de `boa_engine` en esta configuración (todas permisivas; las versiones exactas, en `Cargo.lock`):

| Licencia | Crates |
| --- | --- |
| Unlicense o MIT | `boa_*` (7), `memchr`, `small_btree`, `tag_ptr` |
| MIT o Apache-2.0 | `allocator-api2`, `arrayvec`, `bitflags`, `cfg-if`, `chacha20`, `cpufeatures`, `crossbeam-utils`, `deranged`, `displaydoc`, `dynify`, `either`, `fast-float2`, `fixedbitset`, `futures-concurrency`, `futures-core`, `futures-io`, `getrandom`, `hashbrown`, `heck`, `intrusive-collections`, `itertools`, `itoa`, `libc`, `lock_api`, `num-bigint`, `num-conv`, `num-integer`, `num-traits`, `num_threads`, `once_cell`, `oneshot`, `parking_lot_core`, `pastey`, `portable-atomic`, `powerfmt`, `proc-macro-crate`, `proc-macro2`, `quote`, `rand`, `rand_core`, `regress`, `rustc-hash`, `rustversion`, `scopeguard`, `serde` y derivados, `serde_json`, `siphasher`, `smallvec`, `stable_deref_trait`, `static_assertions`, `syn`, `thin-vec`, `thiserror`, `time` y derivados, `toml_*`, `base64` |
| Apache-2.0 o MIT | `async-channel`, `concurrent-queue`, `equivalent`, `event-listener`, `event-listener-strategy`, `fastrand`, `futures-lite`, `indexmap`, `parking`, `pin-project` y derivados, `utf16_iter`, `utf8_iter`, `utf_types`, `write16` |
| MIT | `aligned-vec`, `cow-utils`, `dashmap`, `equator`, `phf` y derivados, `strum`, `synstructure`, `tap`, `winnow`, `zmij` |
| Unicode-3.0 | `icu_collections`, `icu_locale_core`, `icu_normalizer`, `icu_normalizer_data`, `icu_properties`, `icu_properties_data`, `icu_provider`, `litemap`, `potential_utf`, `tinystr`, `writeable`, `yoke` y derivados, `zerofrom` y derivados, `zerotrie`, `zerovec` y derivados |
| Otras permisivas | `foldhash` (Zlib), `bytemuck` (Zlib, Apache-2.0 o MIT), `num_enum` (BSD-3-Clause, MIT o Apache-2.0), `ryu-js` (Apache-2.0 o BSL-1.0), `unicode-ident` ((MIT o Apache-2.0) y Unicode-3.0) |

Los datos de Unicode (`icu_*_data`, `unicode-ident`) están bajo la licencia Unicode-3.0, que pide conservar su aviso de
copyright: va en `licenses\` con el resto.

## Crates con MPL-2.0 (copyleft por archivo)

`cssparser`, `cssparser-macros`, `dtoa-short`, `selectors` y `option-ext` (solo en `pliego-app`, vía Tauri) están bajo
**MPL-2.0**. La MPL-2.0 exige que quien reciba el ejecutable pueda obtener el código fuente **de esos archivos**; Pliego no
los modifica, y su código fuente está en crates.io con la versión de `Cargo.lock`. No afecta al resto del código de
Pliego, que no se mezcla con esos archivos. `htmlescape` ofrece MPL-2.0 entre varias licencias; Pliego lo usa bajo la MIT.

## Independent JPEG Group (IJG)

Este software se basa en parte en el trabajo del Independent JPEG Group.

Afecta a `jpeg-encoder` ((MIT o Apache-2.0) y IJG; codificación JPEG en `pliego-optimize`) y a la libjpeg-turbo que incluye
PDFium.

## Tesseract, Leptonica y sus dependencias (DLL del instalador)

`pliego ocr` usa Tesseract 5 (Apache-2.0) y Leptonica (BSD-2-Clause) como DLL de vcpkg, que `installer/setup/build.ps1`
copia junto a `pliego.exe` con sus dependencias (libpng, zlib, libtiff, libjpeg, libwebp, openjpeg, giflib…, todas
permisivas). El texto de licencia de cada una va en `licenses\` dentro de la carpeta de la versión. Los datos de idioma
de Tesseract (`tessdata`, Apache-2.0) se descargan aparte, a petición de la persona usuaria.

## rustybuzz, ttf-parser y subsetter

`subsetter` (MIT o Apache-2.0, de Typst) reduce la fuente incrustada a los glifos usados. `rustybuzz` (MIT, port a Rust de HarfBuzz) y `ttf-parser` (MIT o Apache-2.0) dan forma al texto Unicode que
`pliego-edit` compone con fuentes no estándar. Sus dependencias (`unicode-script`, `unicode-ccc`,
`unicode-bidi-mirroring`, `core_maths`) son MIT o Apache-2.0.

## Liberation Fonts 2.1.5 (incluidas, SIL OFL-1.1)

Los 12 archivos `.ttf` de `crates/pliego-pdf/fonts/liberation/` son los de Liberation Fonts 2.1.5, sin modificar, bajo la
SIL Open Font License 1.1 (texto en esa carpeta). Miden y dibujan las 14 fuentes estándar de PDF. Condiciones: no se
venden por separado, se distribuyen con su aviso y licencia, y una versión modificada no puede llevar los nombres
reservados «Liberation», «Arimo», «Tinos» o «Cousine». La OFL **no** se extiende al resto de Pliego.

## Noto Sans CJK (descarga opcional, SIL OFL-1.1)

Pliego no incluye fuentes CJK en el binario. Para editar texto CJK usa primero las fuentes del sistema (Microsoft
YaHei, Yu Gothic, Malgun Gothic, SimSun) y, si faltan, Noto Sans CJK puede descargarse aparte en
`%LOCALAPPDATA%\Pliego\fonts`. Noto Sans CJK se distribuye bajo la SIL Open Font License 1.1; no forma parte del
código de Pliego ni de sus dependencias de compilación, así que no se añade a `deny.toml`.

## Asistente local: llama.cpp y modelos (opcional)

El asistente usa llama.cpp (MIT) mediante `llama-cpp-2` (MIT o Apache-2.0) solo si se compila con la característica
`llama` (desactivada por defecto). **Pliego no distribuye ningún modelo**: la persona usuaria descarga el que elige de
Hugging Face (catálogo en `crates/pliego-assistant/src/models.rs`, solo licencias Apache-2.0 o MIT, con SHA-256), y la
interfaz muestra la licencia de cada modelo en el catálogo.

## Interfaz (npm, `app/`)

Dependencias de producción incluidas en el paquete de la interfaz: `solid-js` 1.9 (MIT), `lucide-solid` 1.66 (ISC; los
iconos de Lucide son ISC, con una parte derivada de Feather bajo MIT), `@tauri-apps/api` 2.12 (Apache-2.0 o MIT), `csstype`
(MIT), `seroval` y `seroval-plugins` (MIT). Las de desarrollo (Vite, Vitest, Playwright, ESLint, axe-core con MPL-2.0…)
no se distribuyen.

## Microsoft Edge WebView2 Runtime

La interfaz se muestra con WebView2, que no se incluye: lo instala o ya lo tiene Windows. Se usa bajo los términos de
redistribución de Microsoft.

## Corpus de pruebas CJK (`tests/corpus/edit/corpus/cjk-*.pdf`)

Los tres PDFs de prueba CJK llevan incrustado solo el subconjunto de glifos usado (unos pocos kB) de IPAGothic
(IPA Font License 1.0, japonés) y de WenQuanYi Zen Hei (Apache-2.0 o GPLv2 con excepción de fuentes, chino y
coreano). Son archivos de prueba: no se distribuyen con el binario de Pliego.

## Lo que ya no está

MuPDF (Artifex, AGPL-3.0) y el crate `mupdf`/`mupdf-sys` se retiraron del proyecto en #200 (2026-11-05). `webpki-roots`
(CDLA-Permissive-2.0) y `rustls` también salieron (#198): las descargas usan el almacén de certificados de Windows.
