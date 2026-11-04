/** Estado de la redacción: zonas marcadas a mano y coincidencias de la vista previa, por documento (puntos PDF, origen arriba-izquierda). */
import { createSignal } from "solid-js";

export interface Area { page: number; x0: number; y0: number; x1: number; y1: number }
export interface FoundMatch { page: number; pattern: string; text: string; areas: Area[] }

/** Tamaño mínimo (puntos) para que un arrastre cuente como zona y no como un clic. */
export const MIN_SIDE = 4;

/** Normaliza un arrastre (cualquier dirección) a una zona dentro de la página; `null` si es demasiado pequeña. */
export function toArea(page: number, a: { x: number; y: number }, b: { x: number; y: number }, size: { width: number; height: number }): Area | null {
  const clamp = (v: number, max: number) => Math.min(Math.max(v, 0), max);
  const x0 = clamp(Math.min(a.x, b.x), size.width), x1 = clamp(Math.max(a.x, b.x), size.width);
  const y0 = clamp(Math.min(a.y, b.y), size.height), y1 = clamp(Math.max(a.y, b.y), size.height);
  if (x1 - x0 < MIN_SIDE || y1 - y0 < MIN_SIDE) return null;
  return { page, x0, y0, x1, y1 };
}

const [areaMap, setAreaMap] = createSignal<Record<number, Area[]>>({});
const [matchMap, setMatchMap] = createSignal<Record<number, FoundMatch[]>>({});
const [markingOn, setMarkingOn] = createSignal(false);

export const marking = markingOn;
export const setMarking = setMarkingOn;
export const areasOf = (doc: number): Area[] => areaMap()[doc] ?? [];
export const areasOnPage = (doc: number, page: number): Area[] => areasOf(doc).filter((a) => a.page === page);
export const addArea = (doc: number, area: Area): void => { setAreaMap((m) => ({ ...m, [doc]: [...(m[doc] ?? []), area] })); };
export const clearAreas = (doc: number): void => { setAreaMap((m) => ({ ...m, [doc]: [] })); };
export const removeLastArea = (doc: number): void => { setAreaMap((m) => ({ ...m, [doc]: (m[doc] ?? []).slice(0, -1) })); };

export const matchesOf = (doc: number): FoundMatch[] => matchMap()[doc] ?? [];
export const setMatches = (doc: number, list: FoundMatch[]): void => { setMatchMap((m) => ({ ...m, [doc]: list })); };
export const matchAreasOnPage = (doc: number, page: number): Area[] => matchesOf(doc).flatMap((m) => m.areas).filter((a) => a.page === page);

/** Para pruebas. */
export function resetRedact(): void { setAreaMap({}); setMatchMap({}); setMarkingOn(false); }
