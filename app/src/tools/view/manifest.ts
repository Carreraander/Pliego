import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Herramientas de Ver (W29). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const properties: ToolManifest = {
  id: "viewer.properties", group: "view", kind: "task", title: "tools.viewer.properties.title", description: "tools.viewer.properties.description",
  icon: "info", keywords: ["metadatos", "autor", "info", "propiedades"], shortcut: "Ctrl+D", when: { document: true }, pinnable: true,
  load: () => import("./PropertiesPanel"),
};

export const attachments: ToolManifest = {
  id: "viewer.attachments", group: "view", kind: "task", title: "tools.viewer.attachments.title", description: "tools.viewer.attachments.description",
  icon: "paperclip", keywords: ["adjuntar", "attachments", "incrustados", "archivos"], when: { document: true }, pinnable: true,
  load: () => import("./AttachmentsPanel"),
};

export function registerViewTools(): void {
  registerTool(attachments);
  registerTool(properties);
}
