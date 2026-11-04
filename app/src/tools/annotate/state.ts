import { createSignal } from "solid-js";
import type { CommentTool, Rgb } from "./geometry";
import { COLORS } from "./geometry";

/** Herramienta activa del modo Comentar (`null` = solo mirar). */
export const [commentTool, setCommentTool] = createSignal<CommentTool | null>("highlight");
export const [commentColor, setCommentColor] = createSignal<Rgb>(COLORS[0]!.rgb);
/** Cambios que se pueden deshacer en el documento activo. */
export const [undoDepth, setUndoDepth] = createSignal(0);
