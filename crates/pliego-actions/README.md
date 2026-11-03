# pliego-actions

Acciones en lote: una secuencia de pasos guardada en TOML o JSON que se aplica a PDFs y carpetas.

- Cada paso es `kind = "<módulo>[.<operación>]"` más las opciones serializables de ese módulo.
- Pasos de la CLI: `pliego run --list-steps`. Ejemplo: `examples/archivar.toml`.
- Los originales nunca se tocan: el resultado va a la carpeta de salida con la misma estructura de subcarpetas.
- Tras cada archivo se guarda `.pliego-run.json` en la salida; `--resume` retoma un lote cancelado o interrumpido
  sin repetir los archivos ya procesados (siempre que la acción y el archivo no hayan cambiado).
- El crate no depende de los módulos: quien lo usa registra los pasos en un `Registry` (ver `pliego-cli`).
