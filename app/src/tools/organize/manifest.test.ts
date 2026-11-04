import { describe, expect, it } from "vitest";
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerOrganizeTools } from "./manifest";

describe("herramientas de Organizar", () => {
  it("sustituyen a las fantasma de base.ts", () => {
    registerBaseTools();
    expect(getTool("pages.merge")?.ghost).toBe(true);
    registerOrganizeTools();
    for (const id of ["pages.merge", "pages.split", "pages.insert", "pages.number"]) {
      const t = getTool(id)!;
      expect(t.ghost).toBeFalsy();
      expect(typeof t.load).toBe("function");
    }
  });
  it("Dividir pide un documento y Combinar no", () => {
    registerOrganizeTools();
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(getTool("pages.split")!, none).enabled).toBe(false);
    expect(toolState(getTool("pages.merge")!, none).enabled).toBe(true);
  });
});
