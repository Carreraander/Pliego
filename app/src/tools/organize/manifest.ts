import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Herramientas de Organizar (W23). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const merge: ToolManifest = {
  id: "pages.merge", group: "organize", kind: "task", title: "tools.pages.merge.title", description: "tools.pages.merge.description",
  icon: "combine", keywords: ["unir", "merge", "juntar"], pinnable: true, load: () => import("./MergePanel"),
};
export const split: ToolManifest = {
  id: "pages.split", group: "organize", kind: "task", title: "tools.pages.split.title", description: "tools.pages.split.description",
  icon: "scissors", keywords: ["separar", "split", "partir"], when: { document: true }, pinnable: true, load: () => import("./SplitPanel"),
};
export const insert: ToolManifest = {
  id: "pages.insert", group: "organize", kind: "task", title: "tools.pages.insert.title", description: "tools.pages.insert.description",
  icon: "file-plus", keywords: ["insertar", "añadir", "blanco"], when: { document: true }, pinnable: true, load: () => import("./InsertPanel"),
};
export const number: ToolManifest = {
  id: "pages.number", group: "organize", kind: "task", title: "tools.pages.number.title", description: "tools.pages.number.description",
  icon: "hash", keywords: ["numeración", "paginar", "etiquetas"], when: { document: true }, pinnable: true, load: () => import("./NumberPanel"),
};

export function registerOrganizeTools(): void {
  registerTool(merge);
  registerTool(split);
  registerTool(insert);
  registerTool(number);
}
