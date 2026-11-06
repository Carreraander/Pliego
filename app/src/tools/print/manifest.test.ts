import { describe, expect, it } from "vitest";
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerPrintTools } from "./manifest";
import { pt } from "./strings";

describe("herramienta Imprimir (W44)", () => {
  it("es una tarea del inspector con Ctrl+P que pide un documento", () => {
    registerBaseTools();
    registerPrintTools();
    const t = getTool("print.document")!;
    expect(t.ghost).toBeFalsy();
    expect(t.kind).toBe("task");
    expect(t.group).toBe("print");
    expect(t.shortcut).toBe("Ctrl+P");
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(false);
    expect(toolState(t, { ...none, document: true }).enabled).toBe(true);
  });

  it("los textos sustituyen las variables", () => {
    expect(pt("print.summary", { pages: 3, total: 10, copies: 2 })).toBe("3 de 10 páginas · 2 copia(s)");
    expect(pt("print.err.outside", { n: 7 })).toBe("El documento tiene 7 páginas");
  });
});
