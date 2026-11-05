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

export const layers: ToolManifest = {
  id: "viewer.layers", group: "view", kind: "task", title: "tools.viewer.layers.title", description: "tools.viewer.layers.description",
  icon: "layers", keywords: ["capas", "layers", "ocg", "contenido opcional"], when: { document: true }, pinnable: true,
  load: () => import("./LayersPanel"),
};

export const present: ToolManifest = {
  id: "viewer.present", group: "view", kind: "task", title: "tools.viewer.present.title", description: "tools.viewer.present.description",
  icon: "presentation", keywords: ["pantalla completa", "slides", "fullscreen"], shortcut: "F11", when: { document: true }, pinnable: true,
  load: () => import("./PresentLauncher"),
};

export function registerViewTools(): void {
  registerTool(attachments);
  registerTool(layers);
  registerTool(present);
  registerTool(properties);
}
