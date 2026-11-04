import { describe, expect, it } from "vitest";
import { getTool, NO_DOCUMENT, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerSignTools } from "./manifest";

describe("herramientas de Firmas", () => {
  it("sustituyen a las fantasma de base.ts", () => {
    registerBaseTools();
    expect(getTool("sign.digital")?.ghost).toBe(true);
    registerSignTools();
    for (const id of ["sign.digital", "sign.validate", "sign.workflow"]) expect(getTool(id)?.ghost).toBeFalsy();
    expect(typeof getTool("sign.digital")!.load).toBe("function");
    expect(getTool("sign.validate")!.panelId).toBe("sign.panel");
  });
  it("Validar solo se activa con un documento firmado", () => {
    registerSignTools();
    const validate = getTool("sign.validate")!;
    expect(toolState(validate, NO_DOCUMENT).enabled).toBe(false);
    expect(toolState(validate, { ...NO_DOCUMENT, document: true }).enabled).toBe(false);
    expect(toolState(validate, { ...NO_DOCUMENT, document: true, signatures: true }).enabled).toBe(true);
    expect(toolState(getTool("sign.digital")!, { ...NO_DOCUMENT, document: true }).enabled).toBe(true);
  });
});
