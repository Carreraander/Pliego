import { createSignal, type Component } from "solid-js";
import type { OpenDoc } from "../state/documents";

/** Capa que una herramienta dibuja encima de cada página (dentro de la página sin girar, en píxeles = puntos × zoom). */
export type PageOverlay = Component<{ doc: OpenDoc; index: number; zoom: number }>;

const [list, setList] = createSignal<PageOverlay[]>([]);
export const pageOverlays = list;

/** Registra una capa interactiva sobre las páginas del visor. Devuelve cómo retirarla. */
export function registerPageOverlay(overlay: PageOverlay): () => void {
  setList((all) => [...all, overlay]);
  return () => setList((all) => all.filter((o) => o !== overlay));
}
