/**
 * Lógica pura del modo Organizar: la lista de páginas del resultado (orden, giros, borrados, duplicados) y la selección.
 * No toca el documento: «Guardar copia» envía `toEdits(...)` a `organize_pages`, así que todo se puede deshacer hasta entonces.
 */
export interface PageItem { /** Página del original, desde 0. */ page: number; /** Giro extra en grados (0, 90, 180, 270). */ rotate: number; /** Clave estable para la lista. */ key: number }
export interface Edits { items: PageItem[]; /** Posiciones seleccionadas en `items`. */ selected: number[]; nextKey: number }

export const initialEdits = (pages: number): Edits => ({ items: Array.from({ length: pages }, (_, i) => ({ page: i, rotate: 0, key: i })), selected: [], nextKey: pages });

const sorted = (xs: number[]): number[] => [...new Set(xs)].sort((a, b) => a - b);
const norm = (deg: number): number => ((deg % 360) + 360) % 360;

/** Clic normal: selecciona solo esa; Ctrl: añade o quita; Mayús: rango desde la última seleccionada. */
export function select(e: Edits, index: number, mode: "single" | "toggle" | "range"): Edits {
  if (index < 0 || index >= e.items.length) return e;
  if (mode === "single") return { ...e, selected: [index] };
  if (mode === "toggle") return { ...e, selected: e.selected.includes(index) ? e.selected.filter((i) => i !== index) : sorted([...e.selected, index]) };
  const anchor = e.selected.length ? e.selected[e.selected.length - 1]! : index;
  const [a, b] = anchor < index ? [anchor, index] : [index, anchor];
  return { ...e, selected: sorted([...e.selected, ...Array.from({ length: b - a + 1 }, (_, k) => a + k)]) };
}
export const selectAll = (e: Edits): Edits => ({ ...e, selected: e.items.map((_, i) => i) });
export const clearSelection = (e: Edits): Edits => ({ ...e, selected: [] });

export function rotateSelected(e: Edits, degrees: number): Edits {
  const sel = new Set(e.selected);
  return { ...e, items: e.items.map((it, i) => (sel.has(i) ? { ...it, rotate: norm(it.rotate + degrees) } : it)) };
}

/** Borra las seleccionadas. No deja el documento vacío: si se pidiera borrarlo todo, no hace nada. */
export function deleteSelected(e: Edits): Edits {
  const sel = new Set(e.selected);
  const items = e.items.filter((_, i) => !sel.has(i));
  return items.length === 0 || sel.size === 0 ? e : { ...e, items, selected: [] };
}

/** Duplica cada seleccionada justo después de sí misma; las copias quedan seleccionadas. */
export function duplicateSelected(e: Edits): Edits {
  const sel = new Set(e.selected);
  const items: PageItem[] = [];
  const selected: number[] = [];
  let key = e.nextKey;
  e.items.forEach((it, i) => {
    items.push(it);
    if (sel.has(i)) { selected.push(items.length); items.push({ ...it, key: key++ }); }
  });
  return { items, selected, nextKey: key };
}

/** Mueve las seleccionadas una posición (−1 izquierda, +1 derecha); un bloque pegado al borde no se mueve. */
export function moveSelected(e: Edits, dir: -1 | 1): Edits {
  const keys = new Set(e.selected.map((i) => e.items[i]!.key));
  if (keys.size === 0) return e;
  const items = [...e.items];
  const on = (i: number): boolean => keys.has(items[i]!.key);
  if (dir === -1) {
    for (let i = 1; i < items.length; i++) if (on(i) && !on(i - 1)) [items[i - 1], items[i]] = [items[i]!, items[i - 1]!];
  } else {
    for (let i = items.length - 2; i >= 0; i--) if (on(i) && !on(i + 1)) [items[i], items[i + 1]] = [items[i + 1]!, items[i]!];
  }
  return { ...e, items, selected: items.flatMap((it, i) => (keys.has(it.key) ? [i] : [])) };
}

/** Mueve una página a otra posición (arrastrar y soltar). */
export function movePage(e: Edits, from: number, to: number): Edits {
  if (from === to || from < 0 || to < 0 || from >= e.items.length || to >= e.items.length) return e;
  const items = [...e.items];
  const [it] = items.splice(from, 1);
  items.splice(to, 0, it!);
  return { ...e, items, selected: [to] };
}

export const toEdits = (e: Edits): { page: number; rotate: number }[] => e.items.map((it) => ({ page: it.page, rotate: it.rotate }));
export const selectionEdits = (e: Edits): { page: number; rotate: number }[] => e.selected.map((i) => ({ page: e.items[i]!.page, rotate: e.items[i]!.rotate }));

/** ¿Hay cambios respecto al original (orden, giros, borrados o duplicados)? */
export function isDirty(e: Edits, pages: number): boolean {
  return e.items.length !== pages || e.items.some((it, i) => it.page !== i || it.rotate !== 0);
}
