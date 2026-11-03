import { describe, expect, it } from "vitest";
import { flattenHits, quadBounds, stepIndex } from "./search";

describe("búsqueda", () => {
  it("calcula la caja de un cuadrilátero", () => {
    expect(quadBounds([10, 5, 30, 5, 10, 15, 30, 15])).toEqual({ left: 10, right: 30, top: 5, bottom: 15 });
  });
  it("aplana los resultados conservando el orden", () => {
    const m = flattenHits([
      { page: 0, quads: [[0, 0, 1, 0, 0, 1, 1, 1]] },
      { page: 3, quads: [[0, 0, 2, 0, 0, 2, 2, 2], [5, 5, 6, 5, 5, 6, 6, 6]] },
    ]);
    expect(m.map((x) => x.page)).toEqual([0, 3, 3]);
  });
  it("da la vuelta al navegar", () => {
    expect(stepIndex(2, 3, 1)).toBe(0);
    expect(stepIndex(0, 3, -1)).toBe(2);
    expect(stepIndex(-1, 0, 1)).toBe(-1);
  });
});
