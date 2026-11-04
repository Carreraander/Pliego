import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import { registerMode } from "../../shell";
import { registerPageOverlay } from "../../viewer/overlays";
import { EditLayer } from "./EditLayer";
import { EditToolbar } from "./EditToolbar";
import { et } from "./text";
import "./strings";

/** Modo Editar (W29b): edición de texto en el lienzo. Sustituye a la herramienta «fantasma» `edit.content` y al marcador del modo. */
export const content: ToolManifest = {
  id: "edit.content", group: "edit", kind: "mode", title: "tools.edit.content.title", description: "tools.edit.content.description",
  icon: "type", keywords: ["texto", "imagen", "editar", "párrafo"], when: { document: true }, pinnable: true, shortcut: "E",
};

export function registerEditTools(): void {
  registerTool(content);
  // Orden 50: el quinto de los seis modos fijos (ver `DEFAULT_MODE_IDS`).
  registerMode({ id: content.id, get title() { return et("tools.edit.content.title"); }, icon: content.icon, order: 50, needsDocument: true, toolbar: EditToolbar });
  registerPageOverlay(EditLayer);
}
