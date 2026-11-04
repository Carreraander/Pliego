import { describe, expect, it } from "vitest";
import { stepLabel } from "./actionSteps";

describe("pasos de una acción", () => {
  it("muestra el tipo y las opciones sencillas", () => {
    expect(stepLabel({ kind: "pages.rotate", degrees: 90, pages: "1-3" })).toBe("pages.rotate (degrees = 90, pages = 1-3)");
  });
  it("sin opciones solo el tipo, y se ignoran las opciones anidadas", () => {
    expect(stepLabel({ kind: "optimize" })).toBe("optimize");
    expect(stepLabel({ kind: "protect.encrypt", permissions: { print: true } })).toBe("protect.encrypt");
  });
});
