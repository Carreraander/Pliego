# Componentes de terceros

Pliego es AGPL-3.0-or-later. Estos componentes requieren mención expresa en la documentación.

## Independent JPEG Group (IJG)

Este software se basa en parte en el trabajo del Independent JPEG Group.

Afecta a `jpeg-encoder` (codificación JPEG en `pliego-optimize`) y a la libjpeg que incluye MuPDF.

## rustybuzz, ttf-parser y subsetter

`subsetter` (MIT o Apache-2.0, de Typst) reduce la fuente incrustada a los glifos usados. `rustybuzz` (MIT, port a Rust de HarfBuzz) y `ttf-parser` (MIT o Apache-2.0) dan forma al texto Unicode que
`pliego-edit` compone con fuentes no estándar. Sus dependencias (`unicode-script`, `unicode-ccc`,
`unicode-bidi-mirroring`, `core_maths`) son MIT o Apache-2.0.

## Noto Sans CJK (descarga opcional, SIL OFL-1.1)

Pliego no incluye fuentes CJK en el binario. Para editar texto CJK usa primero las fuentes del sistema (Microsoft
YaHei, Yu Gothic, Malgun Gothic, SimSun) y, si faltan, Noto Sans CJK puede descargarse aparte en
`%LOCALAPPDATA%\Pliego\fonts`. Noto Sans CJK se distribuye bajo la SIL Open Font License 1.1; no forma parte del
código de Pliego ni de sus dependencias de compilación, así que no se añade a `deny.toml`.

## Corpus de pruebas CJK (`tests/corpus/edit/corpus/cjk-*.pdf`)

Los tres PDFs de prueba CJK llevan incrustado solo el subconjunto de glifos usado (unos pocos kB) de IPAGothic
(IPA Font License 1.0, japonés) y de WenQuanYi Zen Hei (Apache-2.0 o GPLv2 con excepción de fuentes, chino y
coreano). Son archivos de prueba: no se distribuyen con el binario de Pliego.
