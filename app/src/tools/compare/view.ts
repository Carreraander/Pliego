/**
 * Vista lado a lado de Comparar (puro, con pruebas): ordena las páginas emparejadas en filas, calcula su disposición
 * y la lista de cambios para saltar de uno a otro. Las dos columnas comparten fila y desplazamiento, así que ir por un
 * documento es ir por el otro.
 */
import { PAGE_GAP, type Layout, type Size } from "../../viewer/layout";

/** Rectángulo en puntos PDF con origen arriba a la izquierda (como el de `pliego_compare::Rect`). */
export interface CmpRect { x0: number; y0: number; x1: number; y1: number }
export type ChangeKind = "added" | "removed" | "changed";
export type PageStatus = "matched" | "inserted" | "deleted";

export interface TextChange {
  kind: ChangeKind;
  page_a: number;
  page_b: number;
  text_a: string;
  text_b: string;
  rects_a: CmpRect[];
  rects_b: CmpRect[];
  format_only: boolean;
  note: string | null;
}
export interface VisualRegion { rect: CmpRect; changed_pixels: number }
export interface PageComparison {
  status: PageStatus;
  page_a: number | null;
  page_b: number | null;
  similarity: number;
  text_changes: TextChange[];
  visual_regions: VisualRegion[];
}
export interface Summary {
  pages_a: number; pages_b: number; inserted_pages: number; deleted_pages: number; changed_pages: number;
  added_words: number; removed_words: number; changed_words: number; format_changes: number; visual_regions: number;
}
export interface Comparison { path_a: string; path_b: string; summary: Summary; pages: PageComparison[] }

/** Disposición de las filas: cada una mide lo que la más alta de sus dos páginas; las dos columnas son igual de anchas. */
export interface RowLayout extends Layout {
  /** Ancho de cada columna (la página más ancha de cualquiera de los dos documentos). */
  columnWidth: number;
}

export function sizeOf(sizes: Size[], page: number | null): Size | null {
  return page === null ? null : (sizes[page] ?? null);
}

export function computeRows(rows: PageComparison[], sizesA: Size[], sizesB: Size[], zoom: number, gap = PAGE_GAP): RowLayout {
  const tops: number[] = [];
  const boxes: Size[] = [];
  let columnWidth = 0;
  for (const r of rows) {
    const a = sizeOf(sizesA, r.page_a);
    const b = sizeOf(sizesB, r.page_b);
    const w = Math.max(a?.width ?? 0, b?.width ?? 0) * zoom;
    const h = Math.max(a?.height ?? 0, b?.height ?? 0) * zoom;
    boxes.push({ width: w, height: h });
    columnWidth = Math.max(columnWidth, w);
  }
  let y = gap;
  for (const b of boxes) { tops.push(y); y += b.height + gap; }
  return { tops, boxes, totalHeight: y, maxWidth: columnWidth * 2, columnWidth };
}

/** Zoom con el que las dos columnas caben en `viewportWidth` (con `margin` de aire en total). */
export function fitColumnsZoom(rows: PageComparison[], sizesA: Size[], sizesB: Size[], viewportWidth: number, margin = 48): number {
  let w = 0;
  for (const r of rows) w = Math.max(w, sizeOf(sizesA, r.page_a)?.width ?? 0, sizeOf(sizesB, r.page_b)?.width ?? 0);
  if (w <= 0) return 1;
  return Math.min(3, Math.max(0.1, (viewportWidth - margin) / (2 * w)));
}

export type StopKind = ChangeKind | "format" | "visual" | "inserted" | "deleted";

/** Un sitio al que saltar: fila, posición vertical (puntos, desde arriba de la página) y de qué tipo es. */
export interface Stop { row: number; y: number; kind: StopKind }

const SAME_LINE = 10; // puntos: lo que cae en el mismo renglón es un solo cambio
const NEAR = 24; // puntos: dos marcas así de cerca en la misma fila cuentan como un solo cambio

const top = (rs: CmpRect[]): number | null => (rs.length ? Math.min(...rs.map((r) => r.y0)) : null);

/** Cambios en orden de lectura: texto, formato, regiones distintas por aspecto y páginas insertadas o eliminadas. */
export function changeStops(c: Comparison): Stop[] {
  const out: Stop[] = [];
  c.pages.forEach((p, row) => {
    if (p.status === "inserted") return void out.push({ row, y: 0, kind: "inserted" });
    if (p.status === "deleted") return void out.push({ row, y: 0, kind: "deleted" });
    const mine: Stop[] = p.text_changes.map((t) => ({ row, y: top(t.rects_b) ?? top(t.rects_a) ?? 0, kind: (t.format_only ? "format" : t.kind) as StopKind }));
    // Las regiones de aspecto que caen sobre un cambio de texto ya contado no son otro cambio.
    for (const v of p.visual_regions) {
      if (!mine.some((m) => m.kind !== "visual" && Math.abs(m.y - v.rect.y0) < NEAR)) mine.push({ row, y: v.rect.y0, kind: "visual" });
    }
    // Lo quitado y lo añadido en el mismo renglón son un solo cambio: se recorren de una vez.
    const kept: Stop[] = [];
    for (const m of mine.sort((x, y) => x.y - y.y)) if (!kept.length || m.y - kept[kept.length - 1]!.y >= SAME_LINE) kept.push(m);
    out.push(...kept);
  });
  return out;
}

/** Marca de lectura: distancia desde el borde de arriba a la que se coloca un cambio al saltar a él. */
export const READ_OFFSET = 64;

/** Posición vertical (px, en el contenido) de un cambio. */
export const stopPx = (s: Stop, tops: number[], zoom: number): number => (tops[s.row] ?? 0) + s.y * zoom;

/** Índice del siguiente (`dir` = 1) o anterior (-1) cambio respecto de `pos` (px del contenido en la marca de lectura). */
export function stepStop(stops: Stop[], tops: number[], zoom: number, pos: number, dir: 1 | -1): number | null {
  if (dir === 1) {
    const i = stops.findIndex((s) => stopPx(s, tops, zoom) > pos + 2);
    return i < 0 ? null : i;
  }
  for (let i = stops.length - 1; i >= 0; i--) if (stopPx(stops[i]!, tops, zoom) < pos - 2) return i;
  return null;
}

/** Cambio en el que está la marca de lectura (el último que no está por debajo de ella), para el contador «3 de 12». */
export function currentStop(stops: Stop[], tops: number[], zoom: number, pos: number): number | null {
  let best: number | null = null;
  for (let i = 0; i < stops.length; i++) if (stopPx(stops[i]!, tops, zoom) <= pos + 2) best = i; else break;
  return best;
}

/** Estilo (en px, dentro de la página a este zoom) de un rectángulo con un pequeño margen para que el color no tape el texto. */
export function rectStyle(r: CmpRect, zoom: number, pad = 1): { left: string; top: string; width: string; height: string } {
  return {
    left: `${r.x0 * zoom - pad}px`, top: `${r.y0 * zoom - pad}px`,
    width: `${(r.x1 - r.x0) * zoom + pad * 2}px`, height: `${(r.y1 - r.y0) * zoom + pad * 2}px`,
  };
}

/** Texto de la leyenda y de los avisos: nombre corto del tipo de cambio. */
export const LEGEND: ChangeKind[] = ["removed", "added", "changed"];
