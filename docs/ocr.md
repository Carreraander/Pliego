# OCR (W06)

`pliego ocr` añade una capa de texto invisible a un PDF escaneado con Tesseract 5, sin recomprimir las imágenes.

```
pliego ocr escaneo.pdf -o escaneo-ocr.pdf --lang spa+eng --deskew
pliego ocr escaneo.pdf --force --rotate        # también páginas con texto y páginas giradas
pliego ocr --list-languages
pliego ocr --download fra                      # descarga verificada (SHA-256) a la carpeta de usuario
```

## Modelos de idioma
Se usan los `tessdata_fast`. Carpetas de búsqueda, por orden: `PLIEGO_TESSDATA`, `%LOCALAPPDATA%\Pliego\tessdata`
(Linux: `~/.local/share/pliego/tessdata`), `TESSDATA_PREFIX` y las rutas del sistema. `spa`, `eng` y `osd` (orientación)
se incluyen en el instalador; los demás idiomas se descargan bajo demanda y solo si están en el catálogo con huella
SHA-256 (`crates/pliego-ocr/src/models.rs`). Una descarga que no coincide se descarta.

## Compilar en Windows (reproducible)
Tesseract se usa como DLL (vcpkg, triplete `x64-windows-release` (DLL, solo release: tarda la mitad)): así no choca con el MuPDF estático (símbolos duplicados y
runtime de MSVC distinto en debug).

```powershell
./tools/ocr/setup-tesseract-windows.ps1 -TessdataDir "$env:LOCALAPPDATA\Pliego\tessdata"
cargo build --release -p pliego-cli
```

El script instala `tesseract` y `leptonica` con vcpkg (`VCPKG_ROOT`, `C:\vcpkg` o lo clona), define
`TESSERACT_LIB_DIR`, `TESSERACT_LIB_NAME`, `LEPTONICA_LIB_NAME` (los lee `crates/tesseract-sys/build.rs`) y añade
`<vcpkg>\installed\x64-windows-release\bin` al `PATH`. **Para empaquetar**, `installer/setup/build.ps1` copia de esa carpeta `bin` junto a `pliego.exe`
las DLL de Tesseract y sus dependencias: `tesseract55.dll`, `leptonica-*.dll` y las que cargan
(`libpng16.dll`, `tiff.dll`, `zlib1.dll`, etc.; `dumpbin /dependents tesseract55.dll` las lista).

En Linux basta `apt install libtesseract-dev libleptonica-dev tesseract-ocr-eng tesseract-ocr-spa tesseract-ocr-osd`.

## Notas de diseño
- `OcrEngine` es un trait: `TesseractEngine` hoy; PaddleOCR/ONNX más adelante sin tocar `ocr_document`.
- Un motor por hilo (`--jobs`, por defecto hasta 4). Tesseract usa OpenMP internamente: si hay muchas páginas en paralelo
  conviene `OMP_THREAD_LIMIT=1` en el entorno.
- La capa usa Courier WinAnsi (sin incrustar). Texto fuera de WinAnsi (alfabetos no latinos) se copia como `?`; migrar a
  una fuente CID con `ToUnicode` está pendiente.
- Reconocimiento a 300 ppp como mínimo; páginas enormes se limitan a 12 000 px de lado.
