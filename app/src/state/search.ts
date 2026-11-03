import { createSignal } from "solid-js";
import { api, type DocId, type SearchHit } from "../api";
import { goToPage } from "./navigation";

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}
export interface Match {
  page: number;
  rect: Rect;
}

/** Caja envolvente de un cuadrilátero [ulx, uly, urx, ury, llx, lly, lrx, lry]. */
export function quadBounds(q: number[]): Rect {
  const xs = [q[0], q[2], q[4], q[6]].map(Number);
  const ys = [q[1], q[3], q[5], q[7]].map(Number);
  return { left: Math.min(...xs), right: Math.max(...xs), top: Math.min(...ys), bottom: Math.max(...ys) };
}

/** Aplana los resultados por página en una lista ordenada de coincidencias. */
export function flattenHits(hits: SearchHit[]): Match[] {
  return hits.flatMap((h) => h.quads.map((q) => ({ page: h.page, rect: quadBounds(q) })));
}

/** Índice siguiente/anterior con vuelta al principio/final. */
export function stepIndex(current: number, total: number, dir: 1 | -1): number {
  if (total === 0) return -1;
  return (current + dir + total) % total;
}

export const [query, setQuery] = createSignal("");
export const [matches, setMatches] = createSignal<Match[]>([]);
export const [matchIndex, setMatchIndex] = createSignal(-1);
export const [searched, setSearched] = createSignal(false);

export function matchesOnPage(page: number): { match: Match; index: number }[] {
  return matches().flatMap((match, index) => (match.page === page ? [{ match, index }] : []));
}

function focusMatch(index: number): void {
  setMatchIndex(index);
  const m = matches()[index];
  if (m) goToPage(m.page, Math.max(0, m.rect.top - 80));
}

export async function runSearch(doc: DocId, text: string): Promise<void> {
  setQuery(text);
  if (!text.trim()) return clearSearch();
  const found = flattenHits(await api.search(doc, text));
  setMatches(found);
  setSearched(true);
  focusMatch(found.length ? 0 : -1);
}

export function nextMatch(dir: 1 | -1): void {
  const i = stepIndex(matchIndex(), matches().length, dir);
  if (i >= 0) focusMatch(i);
}

export function clearSearch(): void {
  setQuery("");
  setMatches([]);
  setMatchIndex(-1);
  setSearched(false);
}
