/** Argumentos de los comandos de Convertir (puro, con pruebas). */

export type ExportFormat = "png" | "jpg" | "tiff" | "svg" | "images" | "txt" | "html";
export type OfficeFormat = "docx" | "xlsx" | "pptx";

export const IMAGE_FORMATS: ExportFormat[] = ["png", "jpg", "tiff"];
/** Formatos que usan resolución. */
export const usesDpi = (f: ExportFormat): boolean => IMAGE_FORMATS.includes(f);

/** Lee un número de un campo; vacío o fuera de rango = `null` (usa el valor por defecto del motor). */
export function optionalNumber(text: string, min: number, max: number): number | null {
  const n = Number(text.trim().replace(",", "."));
  return text.trim() !== "" && Number.isFinite(n) && n >= min && n <= max ? n : null;
}

/** Valida un rango de páginas («all», «1-3,5», «last»); vacío = todas. Devuelve el texto normalizado o `null`. */
export function normalizePages(text: string): string | null {
  const t = text.trim().toLowerCase();
  if (t === "" || t === "all" || t === "todas") return "all";
  if (t === "last") return t;
  return /^\d+(-\d+)?(\s*,\s*\d+(-\d+)?)*$/.test(t) ? t.replace(/\s+/g, "") : null;
}

export function exportArgs(path: string, format: ExportFormat, pages: string, dpi: number | null, jpegQuality: number | null, label: string) {
  return { path, format, pages, dpi: usesDpi(format) ? dpi : null, jpegQuality: format === "jpg" ? jpegQuality : null, password: null, label };
}

export function officeArgs(path: string, format: OfficeFormat, pages: string, failOnScanned: boolean, label: string) {
  return { path, format, pages, imageDpi: null, failOnScanned, password: null, label };
}
