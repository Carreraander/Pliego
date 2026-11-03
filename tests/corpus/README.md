# Corpus de pruebas

- `basic/`, y los demás subdirectorios por módulo: PDF pequeños y generados, incluidos en el repositorio.
- `external/` (ignorado por git): suites públicas de terceros (pdf.js, veraPDF, Isartor).
  Descargar con `python tools/fetch_corpus.py [--suite pdfjs|verapdf|isartor]`.
- `private/` (ignorado por git): archivos propios que no se pueden publicar.

## Ejecutar el corpus externo

```
cargo run -p pliego-qa --release --bin corpus-runner -- tests/corpus/external --timeout 60
PLIEGO_CORPUS=1 cargo test --release -p pliego-qa   # test opt-in (la CI nocturna lo usa)
```

Cada PDF se abre y se renderiza en un proceso hijo con tiempo límite. Resultados: `ok`; `rechazado` (error limpio del
motor, aceptado y listado); `caída` o `bloqueo` (no aceptados: el test falla).

`known-issues.txt` lista los PDF con caída o bloqueo ya conocidos del motor (se listan, no fallan); quitarlos al arreglarlos.
