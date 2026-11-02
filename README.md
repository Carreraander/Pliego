# Pliego

Pliego es una suite PDF libre y gratuita para Windows: ver, anotar, organizar, rellenar y firmar (incluido DNIe),
OCR, proteger, redactar, comparar, optimizar, convertir y validar PDF/A. Ligera (objetivo: instalador < 40 MB) y sin nube.

> Estado: fase 0. Ya existen el núcleo, el motor MuPDF y la CLI básica. La app está en desarrollo.

## Compilar

Requisitos: Rust estable (1.85+), un compilador C (MSVC Build Tools en Windows, clang/gcc en Linux).

```
cargo build --release
target/release/pliego info documento.pdf
target/release/pliego render documento.pdf --pages 1-3 --dpi 150 -o salida/
target/release/pliego text documento.pdf --pages all
target/release/pliego search documento.pdf "factura"
```

## Documentación
- [Arquitectura](docs/ARCHITECTURE.md)
- [Convenciones](docs/CONVENTIONS.md)
- [Flujos de trabajo](docs/workstreams/README.md)

## Licencia
[AGPL-3.0-or-later](LICENSE). Pliego usa [MuPDF](https://mupdf.com/) de Artifex, también AGPL.
