import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Imprimir (W45). Panel del inspector: la regla de diseño es que una tarea no abre un modal (docs/design §4.1). */
export const printDocument: ToolManifest = {
  id: "print.document", group: "print", kind: "task",
  title: "tools.print.document.title", description: "tools.print.document.description",
  icon: "printer", keywords: ["imprimir", "impresora", "print", "papel", "copias"], shortcut: "Ctrl+P", when: { document: true }, pinnable: true,
  load: () => import("./PrintPanel"),
};

export function registerPrintTools(): void {
  registerTool(printDocument);
}
