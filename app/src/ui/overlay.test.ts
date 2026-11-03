import { describe, expect, it } from "vitest";
import { computePointPosition, computePosition } from "./overlay";

const vp = { width: 1000, height: 700 };
const rect = (left: number, top: number, w = 40, h = 30) => ({ left, top, right: left + w, bottom: top + h, width: w, height: h, x: left, y: top, toJSON() {} }) as DOMRect;

describe("computePosition", () => {
  it("coloca debajo y alineado al inicio", () => {
    expect(computePosition(rect(100, 100), { width: 200, height: 100 }, "bottom-start", vp)).toEqual({ x: 100, y: 136 });
  });
  it("voltea hacia arriba si no cabe debajo", () => {
    const p = computePosition(rect(100, 640), { width: 200, height: 100 }, "bottom-start", vp);
    expect(p.y).toBe(640 - 6 - 100);
  });
  it("no se sale por la derecha", () => {
    expect(computePosition(rect(950, 100), { width: 200, height: 100 }, "bottom-start", vp).x).toBe(792);
  });
  it("alinea al final", () => {
    expect(computePosition(rect(500, 100), { width: 200, height: 50 }, "bottom-end", vp).x).toBe(340);
  });
  it("submenú a la derecha, o a la izquierda si no cabe", () => {
    expect(computePosition(rect(100, 100), { width: 200, height: 100 }, "right-start", vp).x).toBe(146);
    expect(computePosition(rect(900, 100), { width: 200, height: 100 }, "right-start", vp).x).toBe(900 - 6 - 200);
  });
});

describe("computePointPosition", () => {
  it("ajusta al borde", () => {
    expect(computePointPosition({ x: 990, y: 690 }, { width: 200, height: 100 }, vp)).toEqual({ x: 792, y: 592 });
  });
});
