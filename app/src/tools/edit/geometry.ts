/** Del gesto del usuario (puntos de página) a las peticiones de `pliego-edit` (ver `app/service/src/pageedit.rs`). */

export type EditTool = "text" | "add";
export type Pt = [number, number];
export type Box = [number, number, number, number];
export type Family = "sans" | "serif" | "mono";
export type Align = "left" | "center" | "right";

/** Reflejo de `pliego_edit::TextBlock` con lo que la interfaz usa. */
export interface TextBlock {
  page: number; id: number; bbox: Box; text: string; lines: string[]; font_name?: string | null; font_size: number;
  bold: boolean; italic: boolean; family: Family;
}

/** Reflejo de `pliego_edit::Style`: lo que falta se toma del bloque original o de un valor por defecto. */
export interface TextStyle { family?: Family; size?: number; bold?: boolean; italic?: boolean; align?: Align }

/** Un arrastre menor que esto (puntos) es un clic y crea un recuadro por defecto. */
export const MIN_BOX = 12;
export const DEFAULT_ADD: [number, number] = [200, 40];

export function boxOf(a: Pt, b: Pt): Box {
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
}

/** Recuadro del texto nuevo: el arrastrado, o uno por defecto desde el punto del clic. */
export function addBox(a: Pt, b: Pt): Box {
  const box = boxOf(a, b);
  if (box[2] - box[0] >= MIN_BOX && box[3] - box[1] >= MIN_BOX) return box;
  return [a[0], a[1], a[0] + DEFAULT_ADD[0], a[1] + DEFAULT_ADD[1]];
}

/** Estilo a enviar: solo lo que el usuario cambió respecto al bloque (para que el resto lo herede). */
export function styleFor(block: TextBlock | null, v: { family: Family; size: number; bold: boolean; italic: boolean; align: Align }): TextStyle {
  const s: TextStyle = { align: v.align };
  if (!block || v.family !== block.family) s.family = v.family;
  if (!block || Math.abs(v.size - block.font_size) > 0.05) s.size = v.size;
  if (!block || v.bold !== block.bold) s.bold = v.bold;
  if (!block || v.italic !== block.italic) s.italic = v.italic;
  return s;
}

/** Tamaño en puntos válido (1–200) o `null`. */
export function parseSize(text: string): number | null {
  const n = Number(text.replace(",", "."));
  return Number.isFinite(n) && n >= 1 && n <= 200 ? n : null;
}

/** El bloque cuyo recuadro contiene el punto, el más pequeño si hay varios. */
export function blockAt(blocks: TextBlock[], pt: Pt): TextBlock | null {
  let best: TextBlock | null = null;
  for (const b of blocks) {
    if (pt[0] < b.bbox[0] || pt[0] > b.bbox[2] || pt[1] < b.bbox[1] || pt[1] > b.bbox[3]) continue;
    const area = (b.bbox[2] - b.bbox[0]) * (b.bbox[3] - b.bbox[1]);
    if (!best || area < (best.bbox[2] - best.bbox[0]) * (best.bbox[3] - best.bbox[1])) best = b;
  }
  return best;
}
