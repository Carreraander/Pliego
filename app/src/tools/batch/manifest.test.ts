import { describe, expect, it } from "vitest";
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerBatchTools } from "./manifest";

describe("herramientas de lotes (W29)", () => {
  it("el índice sustituye a la fantasma y no pide documento", () => {
    registerBaseTools();
    expect(getTool("batch.index")?.ghost).toBe(true);
    registerBatchTools();
    const t = getTool("batch.index")!;
    expect(t.ghost).toBeFalsy();
    expect(typeof t.load).toBe("function");
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(true);
  });
  it("Acciones en lote sustituye a la fantasma y no pide documento", () => {
    registerBaseTools();
    registerBatchTools();
    const t = getTool("batch.actions")!;
    expect(t.ghost).toBeFalsy();
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(true);
  });
  it("Carteras sustituye a la fantasma y tampoco pide documento", () => {
    registerBaseTools();
    registerBatchTools();
    const t = getTool("batch.portfolio")!;
    expect(t.ghost).toBeFalsy();
    expect(t.kind).toBe("task");
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(true);
  });
});
