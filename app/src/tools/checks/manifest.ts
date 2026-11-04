import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Comprobaciones de solo lectura (W29). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
const base = { kind: "task", load: () => import("./ChecksPanel") } as const;

export const accessCheck: ToolManifest = {
  ...base, id: "access.check", group: "standards", title: "tools.access.check.title", description: "tools.access.check.description",
  icon: "accessibility", keywords: ["accesibilidad", "pdf/ua", "etiquetas"], when: { document: true },
};
export const validatePdfa: ToolManifest = {
  ...base, id: "access.validate-pdfa", group: "standards", title: "tools.access.validate-pdfa.title", description: "tools.access.validate-pdfa.description",
  icon: "shield-check", keywords: ["pdf/a", "validar"], when: { document: true },
};
export const preflight: ToolManifest = {
  ...base, id: "print.preflight", group: "print", title: "tools.print.preflight.title", description: "tools.print.preflight.description",
  icon: "clipboard-check", keywords: ["preflight", "imprenta"], when: { document: true },
};

export const readingOrder: ToolManifest = {
  id: "access.order", group: "standards", kind: "task", title: "tools.access.order.title", description: "tools.access.order.description",
  icon: "list-ordered", keywords: ["lectura", "orden", "tabulación"], when: { document: true }, load: () => import("./OrderPanel"),
};

export function registerCheckTools(): void {
  registerTool(accessCheck);
  registerTool(validatePdfa);
  registerTool(preflight);
  registerTool(readingOrder);
}
