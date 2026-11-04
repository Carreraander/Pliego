/** Elemento de la estructura en orden de lectura (lo que devuelve `reading_order`). */
export interface ReadingItem { page: number | null; role: string; has_alt: boolean }

export interface OrderRow { n: number; page: number | null; role: string; noAlt: boolean }

/** Etiquetas que un lector de pantalla necesita describir con texto alternativo. */
const NEEDS_ALT = new Set(["Figure", "Formula"]);

/** Numera los elementos y marca las figuras sin texto alternativo. */
export function orderRows(items: ReadingItem[]): OrderRow[] {
  return items.map((it, i) => ({ n: i + 1, page: it.page, role: it.role, noAlt: NEEDS_ALT.has(it.role) && !it.has_alt }));
}

/** Páginas (desde 1) en las que el orden de lectura vuelve atrás: señal de que el orden puede estar mal. */
export function backwardsPages(items: ReadingItem[]): number[] {
  const out = new Set<number>();
  let last = -1;
  for (const it of items) {
    if (it.page === null) continue;
    if (it.page < last) out.add(it.page + 1);
    last = Math.max(last, it.page);
  }
  return [...out].sort((a, b) => a - b);
}
