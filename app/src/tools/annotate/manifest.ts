import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import { registerMode } from "../../shell";
import { registerPageOverlay } from "../../viewer/overlays";
import { CommentLayer } from "./CommentLayer";
import { CommentToolbar } from "./CommentToolbar";
import { at } from "./text";
import "./strings";

/** Herramientas de Comentar (W26). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const comment: ToolManifest = {
  id: "annotate.comment", group: "comment", kind: "mode", title: "tools.annotate.comment.title", description: "tools.annotate.comment.description",
  icon: "message-square", keywords: ["anotar", "resaltar", "subrayar", "nota", "dibujo", "comentar"], when: { document: true }, pinnable: true,
};
export const xfdf: ToolManifest = {
  id: "annotate.xfdf", group: "comment", kind: "task", title: "tools.annotate.xfdf.title", description: "tools.annotate.xfdf.description",
  icon: "arrow-left-right", keywords: ["xfdf", "comentarios", "importar", "exportar"], when: { document: true }, pinnable: true,
  load: () => import("./XfdfPanel"),
};
export const summary: ToolManifest = {
  id: "annotate.summary", group: "comment", kind: "task", title: "tools.annotate.summary.title", description: "tools.annotate.summary.description",
  icon: "list", keywords: ["resumen", "comentarios", "lista"], when: { document: true }, pinnable: true,
  load: () => import("./SummaryPanel"),
};

export function registerAnnotateTools(): void {
  registerTool(comment);
  registerMode({ id: comment.id, get title() { return at("tools.annotate.comment.title"); }, icon: comment.icon, order: 20, needsDocument: true, toolbar: CommentToolbar });
  registerPageOverlay(CommentLayer);
  registerTool(xfdf);
  registerTool(summary);
}
