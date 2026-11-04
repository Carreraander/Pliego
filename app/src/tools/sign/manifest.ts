import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";
import { PANEL_ID } from "./panel-id";

/** Herramientas de Firmas (W28). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const digital: ToolManifest = {
  id: "sign.digital", group: "fill-sign", kind: "wizard", title: "tools.sign.digital.title", description: "tools.sign.digital.description",
  icon: "sign-digital", keywords: ["certificado", "dnie", "pades", "firmar", "pfx"], when: { document: true }, pinnable: true,
  load: () => import("./SignDigitalPanel"),
};
export const validate: ToolManifest = {
  id: "sign.validate", group: "fill-sign", kind: "panel", panelId: PANEL_ID, title: "tools.sign.validate.title", description: "tools.sign.validate.description",
  icon: "badge-check", keywords: ["firmas", "validar", "comprobar"], when: { document: true, signatures: true }, pinnable: true,
};
export const workflow: ToolManifest = {
  id: "sign.workflow", group: "fill-sign", kind: "wizard", title: "tools.sign.workflow.title", description: "tools.sign.workflow.description",
  icon: "users", keywords: ["pedir firmas", "workflow", "varios firmantes"], when: { document: true }, pinnable: true,
  load: () => import("./WorkflowPanel"),
};

export function registerSignTools(): void {
  registerTool(digital);
  registerTool(validate);
  registerTool(workflow);
}
