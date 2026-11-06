import { createSignal } from "solid-js";
import { api, type DocId } from "../../api";
import { t } from "../../i18n";
import type { OpenDoc } from "../../state/documents";
import { toast } from "../../ui";
import { END, textOfSelection, type Selection, type Word } from "./model";

/** Palabras de cada página (con su revisión: una edición las cambia). Se piden al mostrar la página y se reutilizan al copiar. */
const MAX_CACHED_PAGES = 120;
const cache = new Map<string, Word[]>();
const inflight = new Map<string, Promise<Word[]>>();

const key = (id: DocId, revision: number, page: number) => `${id}:${revision}:${page}`;

/** Palabras ya cargadas de la página, si las hay. */
export function peekWords(doc: Pick<OpenDoc, "id" | "revision">, page: number): Word[] | undefined {
  return cache.get(key(doc.id, doc.revision, page));
}

/** Palabras de la página. Un fallo del motor (o un documento sin texto) da una lista vacía: la capa de texto es un extra, no debe romper la lectura. */
export function loadWords(doc: Pick<OpenDoc, "id" | "revision">, page: number): Promise<Word[]> {
  const k = key(doc.id, doc.revision, page);
  const hit = cache.get(k);
  if (hit) return Promise.resolve(hit);
  const pending = inflight.get(k);
  if (pending) return pending;
  const p = api.pageWords(doc.id, page)
    .then((words) => {
      cache.set(k, words);
      // Fuera las más antiguas (el `Map` conserva el orden de inserción).
      while (cache.size > MAX_CACHED_PAGES) cache.delete(cache.keys().next().value as string);
      return words;
    })
    .catch(() => [] as Word[])
    .finally(() => inflight.delete(k));
  inflight.set(k, p);
  return p;
}

/** Olvida las palabras de un documento (al cerrarlo). */
export function forgetWords(id: DocId): void {
  for (const k of [...cache.keys()]) if (k.startsWith(`${id}:`)) cache.delete(k);
}

/** Solo para pruebas. */
export function resetWords(): void { cache.clear(); inflight.clear(); }

interface Held {
  docId: DocId;
  revision: number;
  sel: Selection;
}
const [held, setHeld] = createSignal<Held | null>(null);

/** Selección del documento, o `null` si no hay o es de otro documento o de otra revisión (el texto pudo cambiar). */
export function selectionOf(doc: Pick<OpenDoc, "id" | "revision">): Selection | null {
  const h = held();
  return h && h.docId === doc.id && h.revision === doc.revision ? h.sel : null;
}

export function select(doc: Pick<OpenDoc, "id" | "revision">, sel: Selection): void {
  setHeld({ docId: doc.id, revision: doc.revision, sel });
}

export function clearSelection(): void { setHeld(null); }

export function selectAll(doc: Pick<OpenDoc, "id" | "revision" | "sizes">): void {
  const last = doc.sizes.length - 1;
  if (last < 0) return;
  select(doc, { anchor: { page: 0, word: 0 }, focus: { page: last, word: END } });
}

/** Texto de la selección; carga las páginas que falten. */
export async function selectedText(doc: Pick<OpenDoc, "id" | "revision">): Promise<string> {
  const sel = selectionOf(doc);
  if (!sel) return "";
  const from = Math.min(sel.anchor.page, sel.focus.page);
  const to = Math.max(sel.anchor.page, sel.focus.page);
  const pages = new Map<number, Word[]>();
  for (let p = from; p <= to; p++) pages.set(p, await loadWords(doc, p));
  return textOfSelection(sel, (p) => pages.get(p) ?? []);
}

async function writeClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Sin permiso del portapapeles asíncrono: la vía clásica, con un área de texto fuera de pantalla.
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.cssText = "position:fixed;left:-9999px;top:0;opacity:0";
    document.body.append(area);
    area.select();
    try { return document.execCommand("copy"); } catch { return false; } finally { area.remove(); }
  }
}

/** Copia la selección al portapapeles y avisa. */
export async function copySelection(doc: Pick<OpenDoc, "id" | "revision">): Promise<boolean> {
  const text = await selectedText(doc);
  if (!text) { toast({ tone: "info", text: t("text.nothingSelected") }); return false; }
  if (await writeClipboard(text)) { toast({ tone: "ok", text: t("text.copied"), timeout: 1800 }); return true; }
  toast({ tone: "danger", text: t("text.copyFailed") });
  return false;
}

export const hasSelection = (doc: Pick<OpenDoc, "id" | "revision">): boolean => selectionOf(doc) !== null;
