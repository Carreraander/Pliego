import { parsePageList } from "./ranges";

const int = (s: string): number | null => { const n = Number(s); return s.trim() !== "" && Number.isInteger(n) ? n : null; };

export interface InsertForm { path: string; total: number; kind: "blank" | "file"; count: string; source: string | null; pages: string; where: string; suffix: string }

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
  return f.source && pages ? { ...base, sourcePath: f.source, pages } : null;
}

export interface NumberForm { path: string; total: number; style: string; prefix: string; first: string; from: string; suffix: string }

/** Argumentos de `number_pages`: una regla desde la página `from` (desde 1). */
export function numberArgs(f: NumberForm): Record<string, unknown> | null {
  const from = int(f.from);
  const first = int(f.first);
  if (!f.path || from === null || from < 1 || from > f.total || first === null || first < 1) return null;
  return { path: f.path, suffix: f.suffix, rules: [{ start_page: from - 1, style: f.style, prefix: f.prefix, first_number: first }] };
}
