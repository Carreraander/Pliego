//! Modelo puro de la capa de texto del visor (sin DOM): qué palabra hay bajo un punto, qué abarca una selección, cómo se
//! pinta y qué texto se copia. Las coordenadas son puntos de página (origen arriba a la izquierda, con el giro de la
//! página aplicado), las mismas que da el worker y que usan las coincidencias de la búsqueda.

export type Box = [x0: number, y0: number, x1: number, y1: number];

/** Palabra de una página, en orden de lectura (lo que devuelve `page_words`). */
export interface Word {
  text: string;
  rect: Box;
  block: number;
  line: number;
}

/** Posición en el documento: página y número de palabra dentro de ella. */
export interface WordPos {
  page: number;
  word: number;
}

export interface Selection {
  anchor: WordPos;
  focus: WordPos;
}

/** Última palabra de una página, para «hasta el final» sin tener que cargarla. */
export const END = Number.MAX_SAFE_INTEGER;

export function comparePos(a: WordPos, b: WordPos): number {
  return a.page - b.page || a.word - b.word;
}

/** Principio y fin de una selección, en orden de lectura. */
export function ordered(sel: Selection): [WordPos, WordPos] {
  return comparePos(sel.anchor, sel.focus) <= 0 ? [sel.anchor, sel.focus] : [sel.focus, sel.anchor];
}

/** Tramo de palabras (ambos extremos incluidos) que la selección ocupa en `page`, o `null` si no la toca. */
export function rangeOnPage(sel: Selection, page: number, count: number): [number, number] | null {
  if (count <= 0) return null;
  const [from, to] = ordered(sel);
  if (page < from.page || page > to.page) return null;
  const a = page === from.page ? Math.min(from.word, count - 1) : 0;
  const b = page === to.page ? Math.min(to.word, count - 1) : count - 1;
  return a <= b ? [a, b] : null;
}

/** Distancia de un punto a una caja (0 si está dentro). */
export function distanceTo(r: Box, x: number, y: number): number {
  const dx = Math.max(r[0] - x, 0, x - r[2]);
  const dy = Math.max(r[1] - y, 0, y - r[3]);
  return Math.hypot(dx, dy);
}

/** Palabra cuya caja contiene el punto (con `pad` puntos de holgura), si la hay. */
export function wordAt(words: Word[], x: number, y: number, pad = 1): number | null {
  let best: number | null = null;
  let bestArea = Infinity;
  for (let i = 0; i < words.length; i++) {
    const r = words[i]!.rect;
    if (x < r[0] - pad || x > r[2] + pad || y < r[1] - pad || y > r[3] + pad) continue;
    const area = (r[2] - r[0]) * (r[3] - r[1]);
    if (area < bestArea) { best = i; bestArea = area; }
  }
  return best;
}

/**
 * Palabra más cercana al punto. Manda la distancia vertical (a la derecha del final de una línea sale la última palabra
 * de esa línea, no la de la línea de abajo) y, a igual altura, la horizontal.
 */
export function nearestWord(words: Word[], x: number, y: number): number | null {
  let best: number | null = null;
  let bestD = Infinity;
  for (let i = 0; i < words.length; i++) {
    const r = words[i]!.rect;
    const dx = Math.max(r[0] - x, 0, x - r[2]);
    const dy = Math.max(r[1] - y, 0, y - r[3]);
    const d = dy * 1e4 + dx;
    if (d < bestD) { best = i; bestD = d; }
  }
  return best;
}

const sameLine = (a: Word, b: Word): boolean => a.block === b.block && a.line === b.line;

/** Palabras de la línea de `i`: primera y última. */
export function lineRange(words: Word[], i: number): [number, number] {
  const w = words[i];
  if (!w) return [i, i];
  let a = i;
  let b = i;
  while (a > 0 && sameLine(words[a - 1]!, w)) a--;
  while (b < words.length - 1 && sameLine(words[b + 1]!, w)) b++;
  return [a, b];
}

/** Una caja por línea con lo seleccionado de `a` a `b` (la caja cubre también los espacios entre palabras). */
export function highlightRects(words: Word[], a: number, b: number): Box[] {
  const out: Box[] = [];
  let prev: Word | null = null;
  for (let i = Math.max(0, a); i <= Math.min(b, words.length - 1); i++) {
    const w = words[i]!;
    const last = out[out.length - 1];
    if (prev && last && sameLine(prev, w)) {
      last[0] = Math.min(last[0], w.rect[0]);
      last[1] = Math.min(last[1], w.rect[1]);
      last[2] = Math.max(last[2], w.rect[2]);
      last[3] = Math.max(last[3], w.rect[3]);
    } else out.push([...w.rect]);
    prev = w;
  }
  return out;
}

/** Texto de las palabras `a..b`: espacio dentro de la línea, salto de línea entre líneas y línea en blanco entre bloques. */
export function textOfRange(words: Word[], a: number, b: number): string {
  let out = "";
  let prev: Word | null = null;
  for (let i = Math.max(0, a); i <= Math.min(b, words.length - 1); i++) {
    const w = words[i]!;
    if (prev) out += prev.block !== w.block ? "\n\n" : prev.line !== w.line ? "\n" : " ";
    out += w.text;
    prev = w;
  }
  return out;
}

/** Texto de una selección. `pages` da las palabras de cada página (vacío si no tiene texto). Páginas separadas por una línea en blanco. */
export function textOfSelection(sel: Selection, pages: (page: number) => Word[]): string {
  const [from, to] = ordered(sel);
  const parts: string[] = [];
  for (let p = from.page; p <= to.page; p++) {
    const words = pages(p);
    const r = rangeOnPage(sel, p, words.length);
    if (r) parts.push(textOfRange(words, r[0], r[1]));
  }
  return parts.join("\n\n");
}

/** Párrafos de la página (un texto por bloque, líneas unidas con espacio): orden de lectura para los lectores de pantalla. */
export function paragraphs(words: Word[]): string[] {
  const out: string[] = [];
  let block = -1;
  for (const w of words) {
    if (w.block !== block || !out.length) { out.push(w.text); block = w.block; } else out[out.length - 1] += ` ${w.text}`;
  }
  return out;
}

export type Direction = "left" | "right" | "up" | "down";

/**
 * Palabra a la que lleva una flecha desde `i`: izquierda y derecha, la anterior y la siguiente; arriba y abajo, la de la
 * línea anterior o siguiente que cae más cerca en horizontal. `null` en el borde de la página (el que llama pasa a la
 * página vecina).
 */
export function moveWord(words: Word[], i: number, dir: Direction): number | null {
  if (i < 0 || i >= words.length) return null;
  if (dir === "left") return i > 0 ? i - 1 : null;
  if (dir === "right") return i < words.length - 1 ? i + 1 : null;
  const [a, b] = lineRange(words, i);
  const target = dir === "up" ? a - 1 : b + 1;
  if (target < 0 || target >= words.length) return null;
  const [ta, tb] = lineRange(words, target);
  const cx = (words[i]!.rect[0] + words[i]!.rect[2]) / 2;
  let best = ta;
  let bestD = Infinity;
  for (let k = ta; k <= tb; k++) {
    const r = words[k]!.rect;
    const d = distanceTo(r, cx, (r[1] + r[3]) / 2);
    if (d < bestD) { best = k; bestD = d; }
  }
  return best;
}
