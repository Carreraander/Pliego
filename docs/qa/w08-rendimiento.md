# W08 · Rendimiento de comparar y optimizar

Medidas con el binario `pliego` en release (`cargo build --release -p pliego-cli`). PDFs generados con
`tools/gen_w08_bench.py` (no se suben al repo). Cada medida es una sola ejecución.

## Entorno
- Linux x86_64, 4 núcleos (entorno de sesión en la nube). **Pendiente repetir en acr12 (Windows)**.

## Resultados (2026-11-03)

| Caso | Entrada | Tiempo | Memoria pico | Resultado |
| --- | --- | --- | --- | --- |
| `compare` texto + visual (96 ppp) | 2 × 500 págs, 3 MB, 20 págs con cambios | 9,8 s | 110 MB | informe PDF de 10,7 MB |
| `compare --no-visual` | igual | 1,6 s | 98 MB | JSON |
| `optimize --analyze` | escaneo de 70 págs, 211 MB | 0,3 s | 16 MB | reparto por categorías |
| `optimize --preset ebook` | igual (JPEG gris 2480×3508) | 12,1 s | 508 MB | 15,7 MB (−93 %) |
| `optimize --preset sin-perdida` | igual | 1,3 s | 16 MB | 211 MB (sin cambios: ya estaba comprimido) |

## Observaciones
- La comparación visual domina el tiempo de `compare` (render de ambas versiones de cada página emparejada); sin ella,
  500 páginas tardan 1,6 s. Un ajuste fino (`--dpi`) lo acerca al coste del texto.
- `optimize ebook` decodifica y reescala imágenes de a una, pero la caché de MuPDF y los búferes hacen que la memoria
  pico llegue a ~500 MB con 210 MB de imágenes. Aceptable en escritorio; si hiciera falta, vaciar la caché de
  imágenes cada N páginas.
- Con imágenes repetidas (4 JPEG distintos en 70 páginas), la limpieza de duplicados de `sin-perdida` redujo 211 MB a 12 MB.

## Pendiente
- Medir en acr12 (Windows) cuando W10 arregle LLVM, y con escaneos reales (1-bit, color, con máscaras).
- Barrido de resoluciones de la comparación visual (72/96/150 ppp) frente a la calidad de detección.
