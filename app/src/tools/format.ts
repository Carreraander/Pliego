// Formato de tamaños (sin dependencias de la interfaz, para probarlo en Node).
/** Tamaño legible con coma decimal en español y punto en inglés. */
export function formatBytes(bytes: number, decimalComma = true): string {
  const KB = 1024;
  let text: string;
  if (bytes < KB) text = `${bytes} B`;
  else if (bytes < KB * KB) text = `${(bytes / KB).toFixed(1)} KB`;
  else if (bytes < KB ** 3) text = `${(bytes / KB ** 2).toFixed(1)} MB`;
  else text = `${(bytes / KB ** 3).toFixed(1)} GB`;
  return decimalComma ? text.replace(".", ",") : text;
}
