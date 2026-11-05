import { createSignal } from "solid-js";
import type { EditTool } from "./geometry";

/** Herramienta activa del modo Editar: `text` (editar un bloque), `add` (texto nuevo), `image`, `object` (vectores) o `link`. */
export const [editTool, setEditTool] = createSignal<EditTool>("text");
/** Al editar un párrafo, ¿se recoloca lo de debajo y lo que no cabe pasa a la página siguiente? */
export const [reflow, setReflow] = createSignal(true);
