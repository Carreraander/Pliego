# Arnés de comparación de motores y oráculo de redacción (W34)

Evidencia automática para la migración de MuPDF a PDFium + lopdf (ola 5). Vive en `crates/pliego-qa`.
Los motores se eligen por nombre en `crates/pliego-qa/src/engines.rs`: `mupdf` (referencia) y `pdfium` (W37, render y
texto con PDFium sobre la misma capa `pliego-pdf`). `pdfium` busca la biblioteca en `PLIEGO_PDFIUM_DLL` (ruta
absoluta) o junto al ejecutable; `tools/pdfium/fetch-pdfium.ps1` la descarga con la versión y el hash fijados.

## `engine-compare`

```
cargo build --release -p pliego-qa
target/release/engine-compare tests/corpus --reference mupdf --candidate mupdf        # línea base
target/release/engine-compare tests/corpus tests/corpus/external --candidate pdfium    # motor nuevo
# PDFium rasteriza y sustituye fuentes base-14 de otra forma: con los umbrales por defecto (pensados para MuPDF contra
# MuPDF) casi todo sale «fuera de umbral». Para PDFium usa los de W37:
target/release/engine-compare tests/corpus --candidate pdfium --pixel-tolerance 64 --max-diff 0.06
```

Por cada PDF (un proceso hijo con tiempo límite; una caída del motor no tira el arnés):

- páginas (las primeras `--max-pages`, 20) y tamaños;
- render a **72 y 150 ppp** con los dos motores: píxeles distintos (tolerancia por canal `--pixel-tolerance` 16;
  fallo si más de `--max-diff` 0,5 % de la página);
- texto extraído: similitud de palabras (fallo por debajo de 0,98);
- `--validate-dir <dir>`: `qpdf --check` y veraPDF sobre las **salidas** de las herramientas (se omiten si no están
  instalados);
- `--mutate N --seed S`: N copias corrompidas por PDF (bytes cambiados, truncado, trozos duplicados); el candidato no
  debe caer ni colgarse. Determinista según la semilla.

Informes en `target/engine-compare/report.{json,md}`. Sale con error si algo falla y **no** está en
`tests/corpus/engine-diff-known.txt` (el equivalente a `known-issues.txt` para este arnés: un PDF por línea con el
motivo en un comentario). Regla de la ola: si un PR de migración supera el umbral más de lo acordado en esa lista, no se
fusiona y se abre incidencia con el PDF.

Línea base (MuPDF contra MuPDF, 85 PDF de `tests/corpus`): 0 píxeles distintos, 0 diferencias de texto; solo
`tiling-hang.pdf` se cuelga a propósito (conocido). Se ejecuta en cada PR dentro de `cargo test -p pliego-qa` y, con el
corpus externo y el humo de robustez, en la CI nocturna.

### Instalar qpdf y veraPDF en acr12

`tools/qa/instalar-validadores.ps1` los instala sin permisos de administrador en `D:\dev-cache\validadores` (qpdf desde
sus versiones de GitHub; veraPDF con el instalador desatendido, que necesita Java 11+ en el PATH). Después,
`. .\tools\qa\instalar-validadores.ps1 -SoloEntorno` añade `bin\` al PATH de la terminal. Probado en acr12 el 2026-11-05
(qpdf 12.4.2, veraPDF 1.28.2; hashes y resultados en `docs/handoff/flujos/w34.md`). Para pasar varios directorios hay que repetir
la opción: `--validate-dir A --validate-dir B` (un valor por aparición).

## `redaction-oracle`

```
target/release/redaction-oracle redactado.pdf --needle 12345678Z --needle "ES91 2100 0418" [--region 0,50,100,300,130]
```

Verificación obligatoria de cualquier motor que redacte (W38). Código de salida 1 si queda rastro. Vías independientes:

1. texto extraído y búsqueda del motor;
2. `pdftotext` (extractor ajeno, si está instalado);
3. bytes del archivo en ASCII, UTF-16BE y hexadecimal;
4. flujos descomprimidos (Flate), incluidos los de objetos comprimidos;
5. literales de texto de los flujos de contenido, aunque el texto vaya partido por interletraje (`[(Hol)-20(a)] TJ`);
6. `--region pagina,x0,y0,x1,y1` (puntos, origen arriba a la izquierda): la zona renderizada a 150 ppp debe ser de un solo
   color.

Tiene control negativo en las pruebas: sobre el original detecta los datos, y sobre la salida de `pliego-redact` no
encuentra nada. **Límites conocidos:** no descifra (pásale un PDF sin cifrar), no decodifica flujos que no sean Flate
(LZW, ASCII85) ni comprueba que las imágenes se recorten más allá de la región uniforme.

## Fuzzing

El fuzzing con libFuzzer del motor actual sigue en `crates/pliego-engine/fuzz` (`tools/fuzz.sh`, `fuzz.yml`).
`--mutate` es el humo en estable que corre contra cualquier motor; el objetivo de libFuzzer del motor nuevo lo añade
quien lo implemente (W37) copiando ese directorio.

## Modelo de documento nativo (W36)

El arnés compara **render y texto** entre motores; el modelo de documento de `pliego_pdf::native` (lopdf) se valida
con pruebas propias que usan MuPDF como motor de referencia, no con `engine-compare`:

- `cargo test -p pliego-pdf native` abre los 85 PDF de `tests/corpus`, los guarda con cuatro combinaciones de opciones y
  comprueba con MuPDF (páginas y texto) y con qpdf (`qpdf --check`, si está en el `PATH`) que la salida es válida.
- `transcript_matches_mupdf` ejecuta el mismo guion de la API sobre los dos motores y compara el registro línea a línea.
- Pruebas de guardado incremental (prefijo intacto, PDF firmado, cifrados), de cifrado (RC4-40/128, AES-128/256, en las dos
  direcciones) y de robustez (archivos dañados y estructuras hostiles no provocan pánicos ni bucles).

Cuando el motor nuevo de render esté enchufado en `engines::by_name`, `engine-compare` valida además lo que este módulo escribe.

## Redacción con el motor propio (W38)

- `cargo test -p pliego-pdf native::redact` (34 pruebas): casos construidos a mano (texto con `TJ`, `'` y `"`, formularios
  compartidos, imágenes Flate y JPEG, imágenes en línea, vectores, anotaciones y campos, `Tr 3`, capas apagadas, cifrado,
  contenido en matriz con recursos heredados) y dos pruebas sobre los 85 PDF del corpus: **redactar la página entera no deja
  texto extraíble** y **redactar una palabra quita esa palabra y casi nada más**.
- `cargo test -p pliego-redact`: las 9 pruebas de integración corren con los dos motores (`RedactEngine::Mupdf` y `Native`).
- `cargo test -p pliego-qa --test harness oraculo`: el oráculo (texto de MuPDF, `pdftotext`, bytes, flujos descomprimidos y
  literales) acepta el resultado del motor propio sobre `tests/corpus/redact/datos.pdf` con los siete patrones.
- Manual: `PLIEGO_REDACT_ENGINE=native pliego redact ... --apply -o salida.pdf` y luego
  `redaction-oracle salida.pdf --needle ...`.
