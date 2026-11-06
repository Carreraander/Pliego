import { t } from "../../i18n";
import { currentPageIndex } from "../../state/documents";
import { announce, brief } from "../announce";
import type { OpenDoc } from "../../state/documents";
import { pointToPage, rotatePoint, type Layout, type Rotation } from "../layout";
import {
  lineRange, moveWord, nearestWord, wordAt, type Direction, type Selection, type WordPos,
} from "./model";
import { clearSelection, loadWords, peekWords, select, selectionOf } from "./state";

/** Lo que el controlador necesita saber del visor en cada momento (son funciones para leer siempre el valor actual). */
export interface ControllerEnv {
  doc: () => OpenDoc;
  zoom: () => number;
  rotation: () => Rotation;
  layout: () => Layout;
  box: () => HTMLElement;
  content: () => HTMLElement;
  /** `true` solo en el modo Leer: los demás modos tienen sus propias capas y gestos. */
  active: () => boolean;
}

interface Drag {
  id: number;
  startX: number;
  startY: number;
  last: { x: number; y: number };
  detail: number;
  anchor: WordPos | null;
  moved: boolean;
  /** Número de la última petición de extensión: solo vale la más reciente. */
  seq: number;
  timer: ReturnType<typeof setInterval> | null;
}

const DRAG_THRESHOLD = 4;
/** Tiempo máximo entre dos pulsaciones seguidas para contarlas como doble o triple clic. */
const MULTI_CLICK_MS = 450;
const KEYS: Record<string, Direction> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" };

/** Selección de texto del visor: arrastrar, doble y triple clic, Mayús+flechas y Esc. */
export function createSelectionController(env: ControllerEnv) {
  let drag: Drag | null = null;
  /** Último gesto iniciado, haya terminado o no: el soltar el botón llega antes que las palabras de la última posición y ese resultado aún cuenta. */
  let latest: Drag | null = null;
  let hoverFrame = 0;
  // `PointerEvent.detail` vale 0: los clics seguidos se cuentan aquí.
  let lastDown = { time: 0, x: 0, y: 0, count: 0 };

  /** Palabra (y si el punto cae dentro de ella) en el punto de pantalla. */
  async function wordFor(cx: number, cy: number): Promise<{ pos: WordPos; inside: boolean } | null> {
    const r = env.content().getBoundingClientRect();
    const hit = pointToPage(env.layout(), env.doc().sizes, env.zoom(), env.rotation(), r.width, cx - r.left, cy - r.top);
    if (!hit) return null;
    const words = peekWords(env.doc(), hit.page) ?? (await loadWords(env.doc(), hit.page));
    const inside = wordAt(words, hit.x, hit.y);
    const word = inside ?? nearestWord(words, hit.x, hit.y);
    return word === null ? null : { pos: { page: hit.page, word }, inside: inside !== null };
  }

  async function extend(d: Drag): Promise<void> {
    const mine = ++d.seq;
    const r = await wordFor(d.last.x, d.last.y);
    if (!r || latest !== d || mine !== d.seq) return;
    d.anchor ??= r.pos;
    select(env.doc(), { anchor: d.anchor, focus: r.pos });
  }

  /** Mientras se arrastra junto al borde, el visor se desplaza y la selección sigue al cursor. */
  function autoscroll(d: Drag): void {
    const b = env.box().getBoundingClientRect();
    const edge = 36;
    const speed = (dist: number) => Math.min(40, Math.max(2, dist / 2));
    let dx = 0;
    let dy = 0;
    if (d.last.y < b.top + edge) dy = -speed(b.top + edge - d.last.y);
    else if (d.last.y > b.bottom - edge) dy = speed(d.last.y - (b.bottom - edge));
    if (d.last.x < b.left + edge) dx = -speed(b.left + edge - d.last.x);
    else if (d.last.x > b.right - edge) dx = speed(d.last.x - (b.right - edge));
    if (!dx && !dy) return;
    env.box().scrollBy(dx, dy);
    void extend(d);
  }

  function endDrag(d: Drag, e: PointerEvent): void {
    if (d.timer) clearInterval(d.timer);
    drag = null;
    try { env.box().releasePointerCapture(e.pointerId); } catch { /* ya soltado */ }
    // Un clic sin arrastrar quita la selección.
    if (!d.moved && d.detail === 1) clearSelection();
  }

  function onPointerDown(e: PointerEvent): void {
    if (!env.active() || e.button !== 0 || drag) return;
    // Los clics en las barras de desplazamiento no son del texto.
    const area = env.box().getBoundingClientRect();
    if (e.clientX - area.left >= env.box().clientWidth || e.clientY - area.top >= env.box().clientHeight) return;
    // Solo el papel: si el gesto cae sobre una capa de otra herramienta (medir, formularios, enlaces…), es suyo.
    const target = e.target as Element | null;
    if (!target?.matches(".viewer, .viewer-content, .page, .page-rot, .tile")) return;
    e.preventDefault();
    env.box().focus({ preventScroll: true });
    try { env.box().setPointerCapture(e.pointerId); } catch { /* sin captura */ }
    const again = e.timeStamp - lastDown.time < MULTI_CLICK_MS && Math.hypot(e.clientX - lastDown.x, e.clientY - lastDown.y) < DRAG_THRESHOLD * 2;
    lastDown = { time: e.timeStamp, x: e.clientX, y: e.clientY, count: again ? lastDown.count + 1 : 1 };
    const d: Drag = {
      id: e.pointerId, startX: e.clientX, startY: e.clientY, last: { x: e.clientX, y: e.clientY }, detail: lastDown.count,
      anchor: null, moved: false, seq: 0, timer: null,
    };
    drag = d;
    latest = d;
    d.timer = setInterval(() => d.moved && autoscroll(d), 40);
    void wordFor(e.clientX, e.clientY).then((r) => {
      if (latest !== d || !r) return;
      if (d.detail >= 2) {
        // Doble clic: la palabra. Triple clic: la línea.
        if (d.detail === 2 && !r.inside) return;
        const words = peekWords(env.doc(), r.pos.page) ?? [];
        const [a, b] = d.detail === 2 ? [r.pos.word, r.pos.word] : lineRange(words, r.pos.word);
        select(env.doc(), { anchor: { page: r.pos.page, word: a }, focus: { page: r.pos.page, word: b } });
        return;
      }
      d.anchor = r.pos;
      if (d.moved) void extend(d); // el cursor ya se había movido mientras llegaban las palabras
      else if (!drag) clearSelection(); // fue un clic suelto y las palabras llegaron tras soltar
    });
  }

  function onPointerMove(e: PointerEvent): void {
    if (drag) {
      if (e.pointerId !== drag.id) return;
      drag.last = { x: e.clientX, y: e.clientY };
      if (!drag.moved && Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY) < DRAG_THRESHOLD) return;
      drag.moved = true;
      if (drag.detail === 1) void extend(drag);
      return;
    }
    // Sin arrastrar: cursor de texto solo sobre las palabras.
    const box = env.box();
    if (!env.active()) { box.removeAttribute("data-over-text"); return; }
    const { clientX, clientY } = e;
    cancelAnimationFrame(hoverFrame);
    hoverFrame = requestAnimationFrame(() => {
      const r = env.content().getBoundingClientRect();
      const hit = pointToPage(env.layout(), env.doc().sizes, env.zoom(), env.rotation(), r.width, clientX - r.left, clientY - r.top);
      const words = hit ? peekWords(env.doc(), hit.page) : undefined;
      const over = !!hit && !!words && wordAt(words, hit.x, hit.y) !== null;
      box.toggleAttribute("data-over-text", over);
    });
  }

  function onPointerUp(e: PointerEvent): void {
    if (drag && e.pointerId === drag.id) endDrag(drag, e);
  }

  /** Lleva el visor hasta la palabra si queda fuera de la ventana. */
  function reveal(pos: WordPos): void {
    const doc = env.doc();
    const rect = peekWords(doc, pos.page)?.[pos.word]?.rect;
    const size = doc.sizes[pos.page];
    if (!rect || !size) return;
    const zoom = env.zoom();
    const page = { width: size.width * zoom, height: size.height * zoom };
    const a = rotatePoint({ x: rect[0] * zoom, y: rect[1] * zoom }, page, env.rotation());
    const b = rotatePoint({ x: rect[2] * zoom, y: rect[3] * zoom }, page, env.rotation());
    const top = env.layout().tops[pos.page]! + Math.min(a.y, b.y);
    const bottom = env.layout().tops[pos.page]! + Math.max(a.y, b.y);
    const box = env.box();
    const margin = 40;
    if (top < box.scrollTop + margin) box.scrollTop = Math.max(0, top - margin);
    else if (bottom > box.scrollTop + box.clientHeight - margin) box.scrollTop = bottom - box.clientHeight + margin;
  }

  /** Mueve el extremo de la selección con una flecha (con Mayús); sin selección, empieza en la página actual. */
  async function extendByKey(dir: Direction): Promise<void> {
    const doc = env.doc();
    let sel: Selection | null = selectionOf(doc);
    if (!sel) {
      const page = currentPageIndex();
      if (!(await loadWords(doc, page)).length) return;
      sel = { anchor: { page, word: 0 }, focus: { page, word: 0 } };
    }
    const from = sel.focus;
    const words = await loadWords(doc, from.page);
    const forward = dir === "right" || dir === "down";
    const next = moveWord(words, Math.min(from.word, words.length - 1), dir);
    let pos: WordPos | null = next === null ? null : { page: from.page, word: next };
    // En el borde de la página se pasa a la siguiente (o anterior) que tenga texto.
    for (let p = from.page + (forward ? 1 : -1); !pos && p >= 0 && p < doc.sizes.length; p += forward ? 1 : -1) {
      const w = await loadWords(doc, p);
      if (w.length) pos = { page: p, word: forward ? 0 : w.length - 1 };
    }
    if (!pos) return;
    select(doc, { anchor: sel.anchor, focus: pos });
    reveal(pos);
    // Sin caret visible, el Narrador no sabe qué se acaba de seleccionar: se lo decimos.
    const word = (peekWords(doc, pos.page) ?? words)[pos.word]?.text;
    if (word) announce(t("viewer.selected", { text: brief(word) }));
  }

  function onKeyDown(e: KeyboardEvent): void {
    if (!env.active() || e.isComposing) return;
    if (e.key === "Escape" && selectionOf(env.doc())) { clearSelection(); return; }
    const dir = KEYS[e.key];
    if (!dir || !e.shiftKey || e.ctrlKey || e.altKey || e.metaKey) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, select, [contenteditable]")) return;
    e.preventDefault();
    void extendByKey(dir);
  }

  return { onPointerDown, onPointerMove, onPointerUp, onPointerCancel: onPointerUp, onKeyDown, extendByKey };
}
