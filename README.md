# Pliego

Pliego es una suite PDF gratuita y con el código a la vista para Windows: ver, anotar, organizar, rellenar y firmar (incluido DNIe),
OCR, proteger, redactar, comparar, optimizar, convertir y validar PDF/A. Ligera (objetivo: instalador < 40 MB) y sin nube.

> Estado: beta. Núcleo en Rust sobre lopdf y PDFium, CLI `pliego` y app de escritorio (Tauri) en desarrollo.

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
Pliego se publica con la [Business Source License 1.1](LICENSE) (código visible, **no** es software libre ni open source
según la OSI):

- **Usarlo es gratis**, también en una empresa, sin límite de usuarios ni de documentos.
- **Vender Pliego o un derivado, integrarlo en un producto de pago u ofrecerlo como servicio de pago requiere una licencia
  comercial.** Las condiciones exactas son las del apartado «Additional Use Grant» de [LICENSE](LICENSE).
- Cada versión pasa a la [AGPL-3.0-or-later](LICENSES/AGPL-3.0-or-later.txt) a los cuatro años de publicarse.
- Las versiones publicadas antes del cambio de licencia siguen bajo la AGPL-3.0-or-later para quien ya las tiene.
- Si quieres contribuir, lee [CONTRIBUTING.md](CONTRIBUTING.md) y el [acuerdo de contribución](CLA.md).

Componentes de terceros (PDFium, lopdf, Boa, Tesseract…): [THIRD-PARTY.md](THIRD-PARTY.md).
