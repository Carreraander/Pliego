import { parsePageList } from "./ranges";

const int = (s: string): number | null => { const n = Number(s); return s.trim() !== "" && Number.isInteger(n) ? n : null; };

export interface InsertForm { path: string; total: number; kind: "blank" | "file"; count: string; source: string | null; sourcePassword?: string | null; pages: string; where: string; suffix: string }

/** Argumentos de `insert_pages`, o `null` si el formulario no es válido. `where` es la página (desde 1) antes de la cual se inserta. */
export function insertArgs(f: InsertForm): Record<string, unknown> | null {
  const where = int(f.where);
  if (!f.path || where === null || where < 1 || where > f.total + 1) return null;
  const base = { path: f.path, at: where - 1, suffix: f.suffix };
  if (f.kind === "blank") {
    const n = int(f.count);
    return n !== null && n >= 1 && n <= 1000 ? { ...base, blank: n } : null;
  }
  const pages = parsePageList(f.pages);
  return f.source && pages ? { ...base, sourcePath: f.source, sourcePassword: f.sourcePassword ?? null, pages } : null;
}

/** Opciones de `merge_pdfs`: marcadores y la contraseña de cada archivo protegido (solo los que van en la lista). */
export function mergeOptions(paths: string[], bookmarkPerFile: boolean, passwords: Record<string, string>): Record<string, unknown> {
  const file_passwords: Record<string, string> = {};
  for (const p of paths) if (passwords[p]) file_passwords[p] = passwords[p]!;
  return { keep_bookmarks: true, bookmark_per_file: bookmarkPerFile, file_passwords };
}

export type NumberFormat = "number" | "of" | "word";
export const NUMBER_TEMPLATE: Record<NumberFormat, (word: string, of: string) => string> = {
  number: () => "{page}",
  of: (_w, of) => `{page} ${of} {pages}`,
  word: (word) => `${word} {page}`,
};

export interface NumberForm { path: string; total: number; position: string; format: string; from: string; size: string; alsoLabels: boolean; suffix: string }

/** Argumentos de `number_pages`; `format` ya es la plantilla («{page} de {pages}»). `null` si el formulario no es válido. */
export function numberArgs(f: NumberForm): Record<string, unknown> | null {
  const from = int(f.from);
  const size = int(f.size);
  if (!f.path || from === null || from < 1 || from > f.total || size === null || size < 4 || size > 72 || !f.format.includes("{page}")) return null;
  return { path: f.path, suffix: f.suffix, request: { position: f.position, format: f.format, from_page: from - 1, size, also_labels: f.alsoLabels } };
}

/** Milímetros → puntos PDF. */
export const MM_TO_PT = 72 / 25.4;

export interface CropForm { path: string; total: number; pages: string; left: string; top: string; right: string; bottom: string; suffix: string }

/** Argumentos de `crop_pages` (márgenes en mm → puntos; páginas «1-3, 5», vacío = todas). `null` si no es válido. */
export function cropArgs(f: CropForm): Record<string, unknown> | null {
  const mm = [f.left, f.top, f.right, f.bottom].map((s) => (s.trim() === "" ? 0 : Number(s.replace(",", "."))));
  if (!f.path || mm.some((v) => !Number.isFinite(v) || v < 0 || v > 2000) || mm.every((v) => v === 0)) return null;
  const pages = parsePageList(f.pages);
  if (!pages || pages.some((p) => p >= f.total)) return null;
  const [left, top, right, bottom] = mm.map((v) => Math.round(v * MM_TO_PT * 100) / 100) as [number, number, number, number];
  return { path: f.path, pages, margins: { left, top, right, bottom }, suffix: f.suffix };
}
