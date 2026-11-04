import { beforeEach, describe, expect, it } from "vitest";
import { setLocale } from "../../i18n";
import { registerBaseTools } from "../base";
import { getTool, NO_DOCUMENT, setDocFacts, toolState } from "../registry";
import { registerOptimizeTools } from "./manifest";
import { optimizeStrings } from "./strings";

beforeEach(() => { setLocale("es"); setDocFacts(NO_DOCUMENT); registerBaseTools(); });

describe("Comprimir y Optimizar avanzado", () => {
  it("sustituyen a las fantasma y ya no dicen «Disponible más adelante»", () => {
    expect(getTool("optimize.compress")?.ghost).toBe(true);
    registerOptimizeTools();
    for (const id of ["optimize.compress", "optimize.advanced"]) {
      expect(getTool(id)?.ghost).toBeUndefined();
      expect(getTool(id)?.load).toBeTypeOf("function");
      expect(toolState(getTool(id)!)).toEqual({ enabled: false, reason: "Abre un documento para usarla" });
    }
    setDocFacts({ document: true });
    expect(toolState(getTool("optimize.compress")!).enabled).toBe(true);
  });
  it("los textos están en español e inglés", () => {
    expect(Object.keys(optimizeStrings.en).sort()).toEqual(Object.keys(optimizeStrings.es).sort());
  });
});
