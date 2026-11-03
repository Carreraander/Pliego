/** Cálculo de posición para superposiciones ancladas (sin dependencias). Coordenadas de ventana (`position: fixed`). */
export type Placement = "bottom-start" | "bottom-end" | "top-start" | "top-end" | "right-start" | "left-start" | "bottom" | "top";

export interface Point { x: number; y: number }

const GAP = 6;
const MARGIN = 8;

export function computePosition(anchor: DOMRect, size: { width: number; height: number }, placement: Placement, viewport = { width: innerWidth, height: innerHeight }): Point {
  const [side, align = "center"] = placement.split("-") as [string, string?];
  let x: number, y: number;
  const flipY = () => (side === "bottom" ? anchor.bottom + GAP + size.height > viewport.height - MARGIN && anchor.top - GAP - size.height >= MARGIN : side === "top" && anchor.top - GAP - size.height < MARGIN);
  const vertical = side === "top" || side === "bottom";
  if (vertical) {
    const below = flipY() ? side !== "bottom" : side === "bottom";
    y = below ? anchor.bottom + GAP : anchor.top - GAP - size.height;
    x = align === "start" ? anchor.left : align === "end" ? anchor.right - size.width : anchor.left + anchor.width / 2 - size.width / 2;
  } else {
    const wantRight = side === "right";
    const fitsRight = anchor.right + GAP + size.width <= viewport.width - MARGIN;
    const fitsLeft = anchor.left - GAP - size.width >= MARGIN;
    const right = wantRight ? fitsRight || !fitsLeft : !fitsLeft && fitsRight;
    x = right ? anchor.right + GAP : anchor.left - GAP - size.width;
    y = anchor.top;
  }
  x = Math.min(Math.max(MARGIN, x), Math.max(MARGIN, viewport.width - size.width - MARGIN));
  y = Math.min(Math.max(MARGIN, y), Math.max(MARGIN, viewport.height - size.height - MARGIN));
  return { x, y };
}

/** Posiciona un menú contextual en el punto del clic, ajustándolo para que quepa. */
export function computePointPosition(point: Point, size: { width: number; height: number }, viewport = { width: innerWidth, height: innerHeight }): Point {
  return {
    x: Math.max(MARGIN, Math.min(point.x, viewport.width - size.width - MARGIN)),
    y: Math.max(MARGIN, Math.min(point.y, viewport.height - size.height - MARGIN)),
  };
}
