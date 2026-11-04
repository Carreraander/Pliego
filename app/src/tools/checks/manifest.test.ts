import { describe, expect, it } from "vitest";
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerCheckTools } from "./manifest";

const IDS = ["access.check", "access.validate-pdfa", "print.preflight", "access.order", "print.preview"];

describe("comprobaciones (W29)", () => {
  it("sustituyen a las fantasma de base.ts", () => {
    registerBaseTools();
    for (const id of IDS) expect(getTool(id)?.ghost).toBe(true);
    registerCheckTools();
    for (const id of IDS) {
      const t = getTool(id)!;
      expect(t.ghost).toBeFalsy();
      expect(typeof t.load).toBe("function");
    }
  });
  it("piden un documento abierto", () => {
    registerCheckTools();
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    for (const id of IDS) expect(toolState(getTool(id)!, none).enabled).toBe(false);
  });
});
