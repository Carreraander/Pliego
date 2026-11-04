/** Preajustes de calidad de Comprimir y paso a los argumentos del comando `compress_pdf` (puro, con pruebas). */

export type CompressQuality = "max" | "balanced" | "min";
export type OptimizePreset = "pantalla" | "ebook" | "impresion" | "sin-perdida";

export const QUALITY_PRESET: Record<CompressQuality, OptimizePreset> = {
  max: "impresion",
  balanced: "ebook",
  min: "pantalla",
};

export type CompressArgs = {
  path: string;
  preset: OptimizePreset;
  dpi: number | null;
  jpegQuality: number | null;
  password: null;
  suffix: string;
};

export function compressArgs(path: string, preset: OptimizePreset, suffix: string, dpi?: number | null, jpegQuality?: number | null): CompressArgs {
  return { path, preset, dpi: dpi ?? null, jpegQuality: jpegQuality ?? null, password: null, suffix };
}

/** Lee un número de un campo de texto; vacío o fuera de rango = «usar el del preajuste» (`null`). */
export function parseOptional(text: string, min: number, max: number): number | null {
  const n = Number(text.trim().replace(",", "."));
  return text.trim() !== "" && Number.isFinite(n) && n >= min && n <= max ? n : null;
}
