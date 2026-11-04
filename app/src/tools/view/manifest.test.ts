import { describe, expect, it } from "vitest";
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerViewTools } from "./manifest";

describe("herramientas de Ver (W29)", () => {
  it("Propiedades sustituye a la fantasma y pide un documento", () => {
    registerBaseTools();
    expect(getTool("viewer.properties")?.ghost).toBe(true);
    registerViewTools();
    const t = getTool("viewer.properties")!;
    expect(t.ghost).toBeFalsy();
    expect(t.shortcut).toBe("Ctrl+D");
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(false);
  });
  it("Adjuntos sustituye a la fantasma (tarea del inspector) y pide un documento", () => {
    registerBaseTools();
    registerViewTools();
    const t = getTool("viewer.attachments")!;
    expect(t.ghost).toBeFalsy();
    expect(t.kind).toBe("task");
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(false);
  });
  it("Capas sustituye a la fantasma y pide un documento", () => {
    registerBaseTools();
    registerViewTools();
    const t = getTool("viewer.layers")!;
    expect(t.ghost).toBeFalsy();
    expect(t.kind).toBe("task");
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(false);
  });
});
