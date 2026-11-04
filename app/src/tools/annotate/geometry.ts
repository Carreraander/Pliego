/** Del gesto del usuario (en puntos de página) a la `Geometry` de `pliego_annot` y el estilo con que se crea. */

export type CommentTool = "highlight" | "underline" | "strike" | "note" | "ink";
export type Pt = [number, number];
export type Rgb = [number, number, number];

export interface Rect { x0: number; y0: number; x1: number; y1: number }

/** Rectángulo normalizado entre dos esquinas arrastradas. */
export function rectOf(a: Pt, b: Pt): Rect {
  return { x0: Math.min(a[0], b[0]), y0: Math.min(a[1], b[1]), x1: Math.max(a[0], b[0]), y1: Math.max(a[1], b[1]) };
}

/** Cuadrilátero `[ul, ur, ll, lr]` de un rectángulo, en el orden que espera `QuadPts`. */
export function quadOf(r: Rect): number[] {
  return [r.x0, r.y0, r.x1, r.y0, r.x0, r.y1, r.x1, r.y1];
}

/** Un arrastre más pequeño que esto (en puntos) se toma como un clic y no crea marca de texto. */
export const MIN_DRAG = 3;

export function isDrag(a: Pt, b: Pt): boolean {
  return Math.abs(a[0] - b[0]) >= MIN_DRAG || Math.abs(a[1] - b[1]) >= MIN_DRAG;
}

export const COLORS: { id: string; rgb: Rgb }[] = [
  { id: "yellow", rgb: [1, 0.9, 0.2] },
  { id: "green", rgb: [0.4, 0.8, 0.4] },
  { id: "blue", rgb: [0.3, 0.6, 1] },
  { id: "pink", rgb: [1, 0.45, 0.7] },
  { id: "red", rgb: [0.9, 0.2, 0.2] },
];

/** Especificación para el comando `annot_add`, o `null` si el gesto no basta para crear nada. */
export function specFor(tool: CommentTool, page: number, color: Rgb, gesture: { points: Pt[] }, contents?: string): Record<string, unknown> | null {
  const pts = gesture.points;
  const first = pts[0];
  const last = pts[pts.length - 1];
  if (!first || !last) return null;
  const style: Record<string, unknown> = { color };
  switch (tool) {
    case "highlight":
    case "underline":
    case "strike": {
      if (!isDrag(first, last)) return null;
      const quads = [quadOf(rectOf(first, last))];
      const type = tool === "strike" ? "strike_out" : tool;
      return { page, geometry: { type, quads }, style: tool === "highlight" ? { ...style, opacity: 0.4 } : style };
    }
    case "note":
      return { page, geometry: { type: "note", at: first }, style, contents: contents ?? "" };
    case "ink":
      if (pts.length < 2) return null;
      return { page, geometry: { type: "ink", strokes: [pts] }, style: { ...style, border_width: 2 } };
  }
}

/** Reduce un trazo a mano alzada: descarta puntos más cerca que `min` del anterior (siempre conserva el último). */
export function thin(points: Pt[], min = 1.5): Pt[] {
  const out: Pt[] = [];
  for (const p of points) {
    const prev = out[out.length - 1];
    if (!prev || Math.hypot(p[0] - prev[0], p[1] - prev[1]) >= min) out.push(p);
  }
  const last = points[points.length - 1];
  if (last && out[out.length - 1] !== last) out.push(last);
  return out;
}
