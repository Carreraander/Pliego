/** Del gesto del usuario (puntos de página) a las peticiones de `pliego-edit` (ver `app/service/src/pageedit.rs`). */

export type EditTool = "text" | "add" | "image" | "object" | "link";
export type Pt = [number, number];
export type Box = [number, number, number, number];
export type Family = "sans" | "serif" | "mono";
export type Align = "left" | "center" | "right";
export type Rgb = [number, number, number];

/** Reflejo de `pliego_edit::TextBlock` con lo que la interfaz usa. */
export interface TextBlock {
  page: number; id: number; bbox: Box; text: string; lines: string[]; font_name?: string | null; font_size: number;
  color?: Rgb; bold: boolean; italic: boolean; family: Family;
}

/** Reflejo de `pliego_edit::Style`: lo que falta se toma del bloque original o de un valor por defecto. */
export interface TextStyle { family?: Family; size?: number; color?: Rgb; bold?: boolean; italic?: boolean; align?: Align }

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
export function styleFor(block: TextBlock | null, v: { family: Family; size: number; bold: boolean; italic: boolean; align: Align; color?: Rgb }): TextStyle {
  const s: TextStyle = { align: v.align };
  if (v.color && (!block?.color || v.color.some((c, i) => Math.abs(c - (block.color?.[i] ?? 0)) > 0.01))) s.color = v.color;
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

// --- Imágenes, objetos vectoriales y enlaces ---

/** Reflejo de `pliego_edit::ImageObject`. */
export interface ImageObj { page: number; id: number; name: string; xref: number; bbox: Box }
/** Reflejo de `pliego_edit::VectorObject`. */
export interface VectorObj { page: number; id: number; bbox: Box }
export type ObjectKind = "image" | "vector";
export type LinkTarget = { type: "uri"; uri: string } | { type: "page"; page: number };
/** Reflejo de `pliego_edit::LinkItem` (`target` nulo si es un destino con nombre u otra acción). */
export interface LinkItem { page: number; id: number; rect: Box; target: LinkTarget | null }
/** Reflejo de `pliego_edit::Transform` (escalar, girar en sentido horario y mover, alrededor del centro). */
export interface Transform { dx: number; dy: number; scale_x: number; scale_y: number; rotate_degrees: number }

export type Handle = "move" | "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";
export const HANDLES: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
/** Lado mínimo (puntos) de una caja al redimensionar. */
export const MIN_SIDE = 4;

/** Caja que resulta de arrastrar `handle` desde `a` hasta `b` (`move` desplaza; los demás mueven su borde o esquina). */
export function dragBox(box: Box, handle: Handle, a: Pt, b: Pt): Box {
  const dx = b[0] - a[0], dy = b[1] - a[1];
  if (handle === "move") return [box[0] + dx, box[1] + dy, box[2] + dx, box[3] + dy];
  let [x0, y0, x1, y1] = box;
  if (handle.includes("w")) x0 = Math.min(x0 + dx, x1 - MIN_SIDE);
  if (handle.includes("e")) x1 = Math.max(x1 + dx, x0 + MIN_SIDE);
  if (handle.includes("n")) y0 = Math.min(y0 + dy, y1 - MIN_SIDE);
  if (handle.includes("s")) y1 = Math.max(y1 + dy, y0 + MIN_SIDE);
  return [x0, y0, x1, y1];
}

/** `Transform` de `pliego-edit` que lleva la caja `from` a la caja `to` (el escalado es respecto al centro). */
export function transformBetween(from: Box, to: Box): Transform {
  const w = from[2] - from[0], h = from[3] - from[1];
  return {
    dx: (to[0] + to[2]) / 2 - (from[0] + from[2]) / 2,
    dy: (to[1] + to[3]) / 2 - (from[1] + from[3]) / 2,
    scale_x: w > 0 ? (to[2] - to[0]) / w : 1,
    scale_y: h > 0 ? (to[3] - to[1]) / h : 1,
    rotate_degrees: 0,
  };
}

export const rotateTransform = (degrees: number): Transform => ({ dx: 0, dy: 0, scale_x: 1, scale_y: 1, rotate_degrees: degrees });

/** ¿El gesto es un cambio real (más de medio punto)? */
export function changed(a: Box, b: Box): boolean {
  return a.some((v, i) => Math.abs(v - (b[i] ?? 0)) > 0.5);
}

/** La caja `inner` recortada al interior de `outer` (para recortar una imagen); `null` si no se solapan. */
export function clampBox(inner: Box, outer: Box): Box | null {
  const r: Box = [Math.max(inner[0], outer[0]), Math.max(inner[1], outer[1]), Math.min(inner[2], outer[2]), Math.min(inner[3], outer[3])];
  return r[2] - r[0] >= MIN_SIDE && r[3] - r[1] >= MIN_SIDE ? r : null;
}

/** Color `#rrggbb` → componentes 0–1. */
export function parseColor(hex: string): Rgb | null {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  return m ? [parseInt(m[1] ?? "0", 16) / 255, parseInt(m[2] ?? "0", 16) / 255, parseInt(m[3] ?? "0", 16) / 255] : null;
}

export function toHex(c: Rgb): string {
  return "#" + c.map((v) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, "0")).join("");
}

/**
 * Destino de un enlace escrito por la persona: un número es una página (1…`pages`); una dirección con esquema
 * (`https:`, `mailto:`…) se usa tal cual; algo con forma de dominio se completa con `https://`. `null` si no vale
 * (las direcciones deben ser ASCII, como exige `pliego-edit`).
 */
export function parseTarget(text: string, pages: number): LinkTarget | null {
  const t = text.trim();
  if (!t) return null;
  if (/^\d+$/.test(t)) {
    const n = Number(t);
    return n >= 1 && n <= pages ? { type: "page", page: n - 1 } : null;
  }
  if (!/^[\x21-\x7e]+$/.test(t)) return null;
  if (/^[a-z][a-z0-9+.-]*:/i.test(t)) return { type: "uri", uri: t };
  return /^[^\s/@]+\.[^\s/]{2,}/.test(t) ? { type: "uri", uri: `https://${t}` } : null;
}

/** Texto que muestra el destino de un enlace en el campo de edición. */
export function targetText(t: LinkTarget | null): string {
  return !t ? "" : t.type === "uri" ? t.uri : String(t.page + 1);
}
