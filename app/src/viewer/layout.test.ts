import { describe, expect, it } from "vitest";
import {
  clampZoom, computeLayout, currentPage, fitPageZoom, fitWidthZoom, lowResZoom, pageBox, rotate, stepZoom, tilesFor,
  unrotateRect, visiblePages,
} from "./layout";

const a4 = { width: 595, height: 842 };

describe("layout", () => {
  it("intercambia ancho y alto al girar", () => {
    expect(pageBox(a4, 2, 90)).toEqual({ width: 1684, height: 1190 });
    expect(pageBox(a4, 1, 180)).toEqual(a4);
  });
  it("apila páginas con separación", () => {
    const l = computeLayout([a4, a4], 1, 0, 10);
    expect(l.tops).toEqual([10, 862]);
    expect(l.totalHeight).toBe(1714);
  });
  it("calcula páginas visibles y la actual", () => {
    const l = computeLayout(Array(100).fill(a4), 1, 0, 10);
    expect(visiblePages(l, 0, 800, 0)).toEqual([0]);
    const v = visiblePages(l, 5000, 800, 400);
    expect(v[0]).toBeGreaterThan(0);
    expect(v.length).toBeLessThan(5);
    expect(currentPage(l, 0, 800)).toBe(0);
    expect(currentPage(l, 852 * 10, 800)).toBe(10);
  });
  it("elige teselas de la zona visible", () => {
    const page = { width: 1190, height: 1684 }; // 3 x 4 teselas
    expect(tilesFor(page, { left: 0, top: 0, right: 100, bottom: 100 })).toEqual([{ col: 0, row: 0 }]);
    expect(tilesFor(page, { left: 500, top: 500, right: 600, bottom: 600 }).length).toBe(4);
    expect(tilesFor(page, { left: 0, top: 0, right: 5000, bottom: 5000 }).length).toBe(12);
  });
  it("ajusta el zoom y lo limita", () => {
    expect(fitWidthZoom(a4, 0, 595 + 32)).toBeCloseTo(1);
    expect(fitPageZoom(a4, 0, { width: 10000, height: 842 + 32 })).toBeCloseTo(1);
    expect(clampZoom(100)).toBe(16);
    expect(clampZoom(0)).toBe(0.1);
  });
  it("recorre los pasos de zoom", () => {
    expect(stepZoom(1, 1)).toBe(1.25);
    expect(stepZoom(1, -1)).toBe(0.75);
    expect(stepZoom(16, 1)).toBe(16);
    expect(stepZoom(0.1, -1)).toBe(0.1);
  });
  it("gira de 90 en 90", () => {
    expect(rotate(270)).toBe(0);
    expect(rotate(0)).toBe(90);
  });
  it("el zoom bajo cabe en una tesela", () => {
    const z = lowResZoom({ width: 2000, height: 3000 });
    expect(2000 * z).toBeLessThan(512);
    expect(3000 * z).toBeLessThan(512);
  });
  it("deshace la rotación de un rectángulo visible", () => {
    const page = { width: 100, height: 200 };
    const r = { left: 0, top: 0, right: 10, bottom: 20 };
    expect(unrotateRect(r, page, 0)).toEqual(r);
    // 90°: la esquina superior izquierda de la vista es la inferior izquierda de la página.
    expect(unrotateRect(r, page, 90)).toEqual({ left: 0, right: 20, top: 190, bottom: 200 });
    expect(unrotateRect(r, page, 180)).toEqual({ left: 90, right: 100, top: 180, bottom: 200 });
    expect(unrotateRect(r, page, 270)).toEqual({ left: 80, right: 100, top: 0, bottom: 10 });
  });
});
