import { goToPage } from "../../state/navigation";
import type { DocId } from "../../api";

/** Lugar de un documento al que se puede volver: página y altura (en puntos desde el borde superior de la página). */
export interface Place {
  page: number;
  y?: number;
}

interface Trail {
  back: Place[];
  forward: Place[];
}

const MAX_TRAIL = 100;
const trails = new Map<DocId, Trail>();
const trail = (id: DocId): Trail => {
  let t = trails.get(id);
  if (!t) trails.set(id, (t = { back: [], forward: [] }));
  return t;
};

/** Dónde está el visor ahora; lo registra el visor montado (devuelve `null` si no hay ninguno). */
let probe: ((id: DocId) => Place | null) | null = null;
export function registerPlaceProbe(fn: (id: DocId) => Place | null): () => void {
  probe = fn;
  return () => { if (probe === fn) probe = null; };
}

const same = (a: Place, b: Place) => a.page === b.page && Math.abs((a.y ?? 0) - (b.y ?? 0)) < 2;

/** Va a `to` apuntando antes dónde se estaba, para poder volver con Atrás. Un salto nuevo borra el «Adelante». */
export function jumpTo(id: DocId, to: Place): void {
  const here = probe?.(id);
  const t = trail(id);
  if (here && !(t.back.length && same(t.back[t.back.length - 1]!, here))) {
    t.back.push(here);
    if (t.back.length > MAX_TRAIL) t.back.shift();
  }
  t.forward = [];
  goToPage(to.page, to.y);
}

export const canGoBack = (id: DocId): boolean => (trails.get(id)?.back.length ?? 0) > 0;
export const canGoForward = (id: DocId): boolean => (trails.get(id)?.forward.length ?? 0) > 0;

/** Vuelve al lugar anterior de la historia de saltos. */
export function goBack(id: DocId): boolean {
  const t = trail(id);
  const to = t.back.pop();
  if (!to) return false;
  const here = probe?.(id);
  if (here) t.forward.push(here);
  goToPage(to.page, to.y);
  return true;
}

/** Rehace un salto deshecho con Atrás. */
export function goForward(id: DocId): boolean {
  const t = trail(id);
  const to = t.forward.pop();
  if (!to) return false;
  const here = probe?.(id);
  if (here) t.back.push(here);
  goToPage(to.page, to.y);
  return true;
}

export function forgetTrail(id: DocId): void { trails.delete(id); }

/** Solo para pruebas. */
export function resetTrails(): void { trails.clear(); probe = null; }
