import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Herramientas de sellado de Editar (W27). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const watermark: ToolManifest = {
  id: "edit.watermark", group: "edit", kind: "task", title: "tools.edit.watermark.title", description: "tools.edit.watermark.description",
  icon: "droplet", keywords: ["watermark", "borrador", "confidencial"], when: { document: true }, pinnable: true, load: () => import("./WatermarkPanel"),
};
export const headerFooter: ToolManifest = {
  id: "edit.headerfooter", group: "edit", kind: "task", title: "tools.edit.headerfooter.title", description: "tools.edit.headerfooter.description",
  icon: "panel-top", keywords: ["header", "footer", "cabecera", "numerar"], when: { document: true }, pinnable: true, load: () => import("./HeaderFooterPanel"),
};
export const background: ToolManifest = {
  id: "edit.background", group: "edit", kind: "task", title: "tools.edit.background.title", description: "tools.edit.background.description",
  icon: "image", keywords: ["background", "color", "imagen"], when: { document: true }, pinnable: true, load: () => import("./BackgroundPanel"),
};
export const bates: ToolManifest = {
  id: "edit.bates", group: "edit", kind: "task", title: "tools.edit.bates.title", description: "tools.edit.bates.description",
  icon: "bates", keywords: ["bates", "legal", "numeración"], when: { document: true }, pinnable: true, load: () => import("./BatesPanel"),
};

export function registerStampTools(): void {
  for (const t of [watermark, headerFooter, background, bates]) registerTool(t);
}
