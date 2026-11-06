//! Cálculo puro de la disposición del visor (sin DOM), para poder probarlo.

export interface Size {
  width: number;
  height: number;
}
export type Rotation = 0 | 90 | 180 | 270;

export const TILE_SIZE = 512;
export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 16;
export const PAGE_GAP = 12;

export function clampZoom(z: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
}

/** Tamaño de la página en píxeles de pantalla (zoom 1 = 72 ppp, es decir, 1 punto = 1 px) con la rotación de vista. */
export function pageBox(size: Size, zoom: number, rotation: Rotation): Size {
  const swap = rotation === 90 || rotation === 270;
  const w = (swap ? size.height : size.width) * zoom;
  const h = (swap ? size.width : size.height) * zoom;
  return { width: w, height: h };
}

export interface Layout {
  tops: number[];
  boxes: Size[];
  totalHeight: number;
  maxWidth: number;
}

export function computeLayout(sizes: Size[], zoom: number, rotation: Rotation, gap = PAGE_GAP): Layout {
  const tops: number[] = [];
  const boxes = sizes.map((s) => pageBox(s, zoom, rotation));
  let y = gap;
  let maxWidth = 0;
  for (const b of boxes) {
    tops.push(y);
    y += b.height + gap;
    maxWidth = Math.max(maxWidth, b.width);
  }
  return { tops, boxes, totalHeight: y, maxWidth };
}

/** Índices de las páginas visibles (con margen `overscan` en px) para un desplazamiento y alto de ventana dados. */
export function visiblePages(layout: Layout, scrollTop: number, viewportHeight: number, overscan = 400): number[] {
  const from = scrollTop - overscan;
  const to = scrollTop + viewportHeight + overscan;
  const out: number[] = [];
  for (let i = 0; i < layout.tops.length; i++) {
    const top = layout.tops[i]!;
    const bottom = top + layout.boxes[i]!.height;
    if (bottom >= from && top <= to) out.push(i);
    if (top > to) break;
  }
  return out;
}

/** Página con mayor parte visible (la "actual"); la más cercana al centro de la ventana. */
export function currentPage(layout: Layout, scrollTop: number, viewportHeight: number): number {
  const center = scrollTop + viewportHeight / 2;
  let best = 0;
  for (let i = 0; i < layout.tops.length; i++) {
    if (layout.tops[i]! <= center) best = i;
    else break;
  }
  return best;
}

export interface TileCoord {
  col: number;
  row: number;
}

/**
 * Teselas que cubren la zona visible de una página. `visibleTop..visibleBottom` y `visibleLeft..visibleRight`
 * están en píxeles de la página SIN rotar al zoom actual.
 */
export function tilesFor(
  page: Size,
  visible: { left: number; top: number; right: number; bottom: number },
  tileSize = TILE_SIZE,
): TileCoord[] {
  const cols = Math.max(1, Math.ceil(page.width / tileSize));
  const rows = Math.max(1, Math.ceil(page.height / tileSize));
  const c0 = Math.max(0, Math.floor(visible.left / tileSize));
  const c1 = Math.min(cols - 1, Math.floor(Math.max(visible.left, visible.right - 1) / tileSize));
  const r0 = Math.max(0, Math.floor(visible.top / tileSize));
  const r1 = Math.min(rows - 1, Math.floor(Math.max(visible.top, visible.bottom - 1) / tileSize));
  const out: TileCoord[] = [];
  for (let r = r0; r <= r1; r++) for (let c = c0; c <= c1; c++) out.push({ col: c, row: r });
  return out;
}

export function fitWidthZoom(page: Size, rotation: Rotation, viewportWidth: number, margin = 32): number {
  const w = pageBox(page, 1, rotation).width;
  return clampZoom((viewportWidth - margin) / w);
}

export function fitPageZoom(page: Size, rotation: Rotation, viewport: Size, margin = 32): number {
  const b = pageBox(page, 1, rotation);
  return clampZoom(Math.min((viewport.width - margin) / b.width, (viewport.height - margin) / b.height));
}

/** Pasos de zoom para los botones acercar/alejar. */
const STEPS = [0.1, 0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4, 6, 8, 12, 16];

export function stepZoom(zoom: number, dir: 1 | -1): number {
  if (dir === 1) return STEPS.find((s) => s > zoom + 1e-6) ?? MAX_ZOOM;
  for (let i = STEPS.length - 1; i >= 0; i--) if (STEPS[i]! < zoom - 1e-6) return STEPS[i]!;
  return MIN_ZOOM;
}

export function rotate(r: Rotation): Rotation {
  return ((r + 90) % 360) as Rotation;
}

/** Zoom bajo para el render progresivo: toda la página cabe en una sola tesela. */
export function lowResZoom(page: Size): number {
  return Math.min(1, (TILE_SIZE - 1) / Math.max(page.width, page.height));
}

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * Convierte un rectángulo expresado en la caja ya girada de la página al sistema de la página sin girar.
 * `page` es el tamaño SIN girar en píxeles; la rotación es en sentido horario.
 */
export function unrotateRect(r: Rect, page: Size, rotation: Rotation): Rect {
  const { width: w, height: h } = page;
  switch (rotation) {
    case 0:
      return r;
    case 90:
      return { left: r.top, right: r.bottom, top: h - r.right, bottom: h - r.left };
    case 180:
      return { left: w - r.right, right: w - r.left, top: h - r.bottom, bottom: h - r.top };
    case 270:
      return { left: w - r.bottom, right: w - r.top, top: r.left, bottom: r.right };
  }
}

/** Transformación CSS (origen arriba a la izquierda) que gira la página sin girar dentro de su caja girada. */
export function rotationTransform(page: Size, rotation: Rotation): string {
  switch (rotation) {
    case 0:
      return "none";
    case 90:
      return `translate(${page.height}px, 0) rotate(90deg)`;
    case 180:
      return `translate(${page.width}px, ${page.height}px) rotate(180deg)`;
    case 270:
      return `translate(0, ${page.width}px) rotate(270deg)`;
  }
}

export interface Pt {
  x: number;
  y: number;
}

/**
 * Inversa de [`rotationTransform`] para un punto: pasa de la caja girada de la página (píxeles) a la página sin girar
 * (píxeles). `page` es el tamaño SIN girar en píxeles.
 */
export function unrotatePoint(p: Pt, page: Size, rotation: Rotation): Pt {
  const { width: w, height: h } = page;
  switch (rotation) {
    case 0:
      return p;
    case 90:
      return { x: p.y, y: h - p.x };
    case 180:
      return { x: w - p.x, y: h - p.y };
    case 270:
      return { x: w - p.y, y: p.x };
  }
}

/** Lo contrario de [`unrotatePoint`]: de la página sin girar a la caja girada. */
export function rotatePoint(p: Pt, page: Size, rotation: Rotation): Pt {
  const { width: w, height: h } = page;
  switch (rotation) {
    case 0:
      return p;
    case 90:
      return { x: h - p.y, y: p.x };
    case 180:
      return { x: w - p.x, y: h - p.y };
    case 270:
      return { x: p.y, y: w - p.x };
  }
}

/**
 * Página bajo un punto del contenido del visor (`x`, `y` en píxeles desde su esquina superior izquierda) y ese punto en
 * puntos de página (sin girar, origen arriba a la izquierda). Entre dos páginas, o fuera del documento, devuelve la
 * página más cercana (arrastrar la selección más allá de los bordes sigue funcionando). `null` si no hay páginas.
 */
export function pointToPage(
  layout: Layout, sizes: Size[], zoom: number, rotation: Rotation, contentWidth: number, x: number, y: number,
  gap = PAGE_GAP,
): { page: number; x: number; y: number } | null {
  if (!layout.tops.length || sizes.length !== layout.tops.length) return null;
  let page = layout.tops.length - 1;
  for (let i = 0; i < layout.tops.length; i++) {
    if (y < layout.tops[i]! + layout.boxes[i]!.height + gap / 2) { page = i; break; }
  }
  const box = layout.boxes[page]!;
  const left = (contentWidth - box.width) / 2;
  const unrotated = { width: sizes[page]!.width * zoom, height: sizes[page]!.height * zoom };
  const p = unrotatePoint({ x: x - left, y: y - layout.tops[page]! }, unrotated, rotation);
  return { page, x: p.x / zoom, y: p.y / zoom };
}
