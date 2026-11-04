import { createSignal } from "solid-js";
import type { EditTool } from "./geometry";

/** Herramienta activa del modo Editar: `text` (editar un bloque) o `add` (texto nuevo). */
export const [editTool, setEditTool] = createSignal<EditTool>("text");
