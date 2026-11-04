/** Interpreta «1-3, 5, 8-10» (páginas desde 1) como tramos `[primera, última]` desde 0, inclusive, que espera `SplitRule::Ranges`. */
export function parseRanges(text: string, pageCount: number): [number, number][] | null {
  const parts = text.split(/[,;]/).map((p) => p.trim()).filter(Boolean);
  if (parts.length === 0) return null;
  const out: [number, number][] = [];
  for (const part of parts) {
    const m = /^(\d+)(?:\s*[-–]\s*(\d+))?$/.exec(part);
    if (!m) return null;
    const a = Number(m[1]);
    const b = m[2] === undefined ? a : Number(m[2]);
    if (a < 1 || b < a || b > pageCount) return null;
    out.push([a - 1, b - 1]);
  }
  return out;
}

/** Lista de páginas «1, 3-5» (desde 1, sin conocer el total) como índices desde 0; vacío = todas; `null` si está mal escrita. */
export function parsePageList(text: string): number[] | null {
  if (text.trim() === "") return [];
  const out: number[] = [];
  for (const part of text.split(/[,;]/).map((p) => p.trim()).filter(Boolean)) {
    const m = /^(\d+)(?:\s*[-–]\s*(\d+))?$/.exec(part);
    if (!m) return null;
    const a = Number(m[1]);
    const b = m[2] === undefined ? a : Number(m[2]);
    if (a < 1 || b < a || b - a > 100000) return null;
    for (let i = a; i <= b; i++) out.push(i - 1);
  }
  return out;
}
