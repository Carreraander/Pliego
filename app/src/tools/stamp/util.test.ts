import { describe, expect, it } from "vitest";
import { hexToRgb, intIn, numIn, pagesOrAll, validPages } from "./util";

describe("util de sellado", () => {
  it("convierte colores", () => {
    expect(hexToRgb("#ff0000")).toEqual([1, 0, 0]);
    expect(hexToRgb("#000000")).toEqual([0, 0, 0]);
    expect(hexToRgb("nada")).toEqual([0, 0, 0]);
  });
  it("valida rangos de páginas", () => {
    for (const ok of ["", "all", "odd", "1-3,5,8-", "2", "last", "1-last", "pares, 1"]) expect(validPages(ok), ok).toBe(true);
    for (const bad of ["x", "1-2-3", "a-b", "1,,", "-3"]) expect(validPages(bad), bad).toBe(false);
    expect(pagesOrAll("  ")).toBe("all");
    expect(pagesOrAll(" 2-4 ")).toBe("2-4");
  });
  it("valida números", () => {
    expect(intIn("6", 1, 12)).toBe(6);
    expect(intIn("0", 1, 12)).toBeNull();
    expect(intIn("2.5", 1, 12)).toBeNull();
    expect(intIn("", 1, 12)).toBeNull();
    expect(numIn("0,5", 0, 1)).toBe(0.5);
    expect(numIn("2", 0, 1)).toBeNull();
  });
});

import { batesSample } from "./util";
describe("Bates", () => {
  it("muestra el primer número", () => {
    expect(batesSample("DOC-", "", 6, 1)).toBe("DOC-000001");
    expect(batesSample("", "/A", 3, 42)).toBe("042/A");
  });
});
