import { beforeEach, describe, expect, it } from "vitest";
import { setLocale } from "../../i18n";
import { registerBaseTools } from "../base";
import { getTool, NO_DOCUMENT, setDocFacts, toolState } from "../registry";
import { registerConvertTools } from "./manifest";

beforeEach(() => { setLocale("es"); setDocFacts(NO_DOCUMENT); registerBaseTools(); });

describe("Convertir", () => {
  it("las herramientas reales sustituyen a las fantasma", () => {
    expect(getTool("convert.export")?.ghost).toBe(true);
    registerConvertTools();
    for (const id of ["convert.export", "convert.office"]) {
      expect(getTool(id)?.ghost).toBeUndefined();
      expect(toolState(getTool(id)!).enabled).toBe(false);
    }
    setDocFacts({ document: true });
    expect(toolState(getTool("convert.office")!).enabled).toBe(true);
  });
});
