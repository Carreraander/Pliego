/** Argumentos de los comandos de Convertir (puro, con pruebas). */

export type ExportFormat = "png" | "jpg" | "tiff" | "images" | "txt" | "html";
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

export type CreateSize = "fit" | "a4" | "a3" | "a5" | "letter" | "legal";
export type CreateOrientation = "auto" | "portrait" | "landscape";
export type PdfaLevel = "1b" | "2b" | "2u" | "3b";

/** Extensiones que «Crear PDF» convierte de una en una (texto y Office); las imágenes se pueden juntar. */
const SINGLE = /\.(txt|md|markdown|html?|docx?|docm|dot|dotx|rtf|odt|xlsx?|xlsm|xlsb|csv|ods|pptx?|pptm|pps|ppsx|odp)$/i;
export const isSingleOnly = (path: string): boolean => SINGLE.test(path);

/** Si la lista no es válida, el motivo (`"single"`: texto u Office junto a otros archivos); si lo es, `null`. */
export function createProblem(paths: string[]): "empty" | "single" | null {
  if (paths.length === 0) return "empty";
  if (paths.length > 1 && paths.some(isSingleOnly)) return "single";
  return null;
}

export function createArgs(paths: string[], pageSize: CreateSize, marginMm: number | null, orientation: CreateOrientation, upscale: boolean, label: string) {
  return { paths, pageSize, marginMm: marginMm ?? 0, orientation, upscale, label };
}

export const pdfaArgs = (path: string, level: PdfaLevel, label: string) => ({ path, level, label });
