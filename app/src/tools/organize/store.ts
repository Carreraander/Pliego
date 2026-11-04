import { createSignal } from "solid-js";
import type { DocId } from "../../api";
import { initialEdits, type Edits } from "./pageedits";

/** Cambios pendientes del modo Organizar por documento. Viven en la interfaz: nada se escribe hasta «Guardar copia». */
const [all, setAll] = createSignal<Record<number, Edits>>({});

export const editsOf = (id: DocId | null, pages: number): Edits => (id === null ? initialEdits(pages) : (all()[id] ?? initialEdits(pages)));
export function updateEdits(id: DocId, pages: number, f: (e: Edits) => Edits): void {
  setAll((m) => ({ ...m, [id]: f(m[id] ?? initialEdits(pages)) }));
}
export function resetEdits(id: DocId): void {
  setAll((m) => Object.fromEntries(Object.entries(m).filter(([k]) => Number(k) !== id)));
}
