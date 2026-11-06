/** Nombre de archivo de una ruta de Windows (`\`) o con `/` (línea de comandos, Git Bash, URL de archivo). */
export function fileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

// Títulos que programas de exportación dejan en los metadatos cuando no hay ninguno de verdad.
const PLACEHOLDER = /^(untitled|sin t[ií]tulo|unbenannt|sans titre|senza titolo)(\b[\s\d._()-]*)?$/i;

/** Lo que se pone en la pestaña: el título del PDF si dice algo; si no, el nombre del archivo. */
export function displayTitle(title: string | null | undefined, path: string): string {
  const t = title?.trim();
  return t && !PLACEHOLDER.test(t) ? t : fileName(path);
}
