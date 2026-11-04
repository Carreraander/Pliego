import { describe, expect, it } from "vitest";
import { addPoint, maxPoints, minPoints, pageDistance, parsePositive, svgPoints } from "./geometry";

describe("medir: geometría", () => {
  it("distancia entre marcas", () => {
    expect(pageDistance([0, 0], [3, 4])).toBe(5);
  });
  it("mínimos y máximos por modo", () => {
    expect(minPoints("distance")).toBe(2);
    expect(minPoints("area")).toBe(3);
    expect(maxPoints("calibrate")).toBe(2);
    expect(maxPoints("area")).toBe(Infinity);
  });
  it("la tercera marca de calibración reinicia la pareja", () => {
    let pts = addPoint([], [1, 1], "calibrate");
    pts = addPoint(pts, [2, 2], "calibrate");
    expect(pts).toHaveLength(2);
    expect(addPoint(pts, [3, 3], "calibrate")).toEqual([[3, 3]]);
    expect(addPoint(pts, [3, 3], "distance")).toHaveLength(3);
  });
  it("números con coma o punto, solo positivos", () => {
    expect(parsePositive("2,5")).toBe(2.5);
    expect(parsePositive(" 10 ")).toBe(10);
    for (const bad of ["", "0", "-3", "abc", "Infinity"]) expect(parsePositive(bad)).toBeNull();
  });
  it("puntos SVG a escala de zoom", () => {
    expect(svgPoints([[1, 2], [3, 4]], 2)).toBe("2,4 6,8");
  });
});
