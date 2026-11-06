import { describe, expect, it } from "vitest";
import {
  changeStops, computeRows, currentStop, fitColumnsZoom, rectStyle, stepStop, stopPx,
  type Comparison, type PageComparison, type TextChange,
} from "./view";

const r = (y0: number, y1 = y0 + 10) => ({ x0: 10, y0, x1: 110, y1 });
const change = (over: Partial<TextChange>): TextChange => ({
  kind: "changed", page_a: 0, page_b: 0, text_a: "a", text_b: "b", rects_a: [r(100)], rects_b: [r(100)], format_only: false, note: null, ...over,
});
const page = (over: Partial<PageComparison>): PageComparison => ({
  status: "matched", page_a: 0, page_b: 0, similarity: 1, text_changes: [], visual_regions: [], ...over,
});
const cmp = (pages: PageComparison[]): Comparison => ({
  path_a: "a.pdf", path_b: "b.pdf", pages,
  summary: { pages_a: 0, pages_b: 0, inserted_pages: 0, deleted_pages: 0, changed_pages: 0, added_words: 0, removed_words: 0, changed_words: 0, format_changes: 0, visual_regions: 0 },
});
const A = [{ width: 600, height: 800 }, { width: 600, height: 800 }];
const B = [{ width: 600, height: 800 }, { width: 700, height: 900 }, { width: 600, height: 800 }];

describe("disposición de la vista lado a lado", () => {
  const rows = [page({ page_a: 0, page_b: 0 }), page({ page_a: 1, page_b: 1 }), page({ status: "inserted", page_a: null, page_b: 2 })];
  it("cada fila mide lo que la más alta de sus dos páginas y las columnas son igual de anchas", () => {
    const l = computeRows(rows, A, B, 0.5);
    expect(l.boxes.map((b) => b.height)).toEqual([400, 450, 400]);
    expect(l.columnWidth).toBe(350);
    expect(l.maxWidth).toBe(700);
    expect(l.tops[0]).toBe(12);
    expect(l.tops[1]).toBe(12 + 400 + 12);
    expect(l.totalHeight).toBe(12 + 400 + 12 + 450 + 12 + 400 + 12);
  });
  it("una página que solo está en un lado ocupa su fila entera", () => {
    const l = computeRows([page({ status: "deleted", page_a: 1, page_b: null })], A, B, 1);
    expect(l.boxes[0]).toEqual({ width: 600, height: 800 });
  });
  it("el zoom de ajuste cabe las dos columnas en la ventana", () => {
    expect(fitColumnsZoom(rows, A, B, 1448)).toBeCloseTo((1448 - 48) / (2 * 700), 6);
    expect(fitColumnsZoom([], A, B, 1000)).toBe(1);
  });
});

describe("cambios para saltar", () => {
  const c = cmp([
    page({ text_changes: [change({ rects_b: [r(300)] }), change({ kind: "added", rects_a: [], rects_b: [r(120)] })] }),
    page({}),
    page({ status: "inserted", page_a: null, page_b: 2 }),
    page({ text_changes: [change({ kind: "removed", rects_b: [], rects_a: [r(40)] })], visual_regions: [{ rect: r(500), changed_pixels: 40 }, { rect: r(42), changed_pixels: 9 }] }),
  ]);
  const stops = changeStops(c);
  it("salen en orden de lectura, con las páginas insertadas, y sin repetir lo que ya es un cambio de texto", () => {
    expect(stops.map((s) => [s.row, s.y, s.kind])).toEqual([
      [0, 120, "added"], [0, 300, "changed"], [2, 0, "inserted"], [3, 40, "removed"], [3, 500, "visual"],
    ]);
  });
  it("lo quitado y lo añadido en el mismo renglón cuentan como un solo cambio", () => {
    const same = cmp([page({ text_changes: [change({ kind: "removed", rects_b: [], rects_a: [r(100)] }), change({ kind: "added", rects_a: [], rects_b: [r(104)] }), change({ rects_a: [r(140)], rects_b: [r(140)] })] })]);
    expect(changeStops(same).map((s) => [s.y, s.kind])).toEqual([[100, "removed"], [140, "changed"]]);
  });
  it("un cambio de solo formato es un tipo aparte", () => {
    expect(changeStops(cmp([page({ text_changes: [change({ format_only: true })] })]))[0]!.kind).toBe("format");
  });
  it("sin diferencias no hay nada que recorrer", () => expect(changeStops(cmp([page({})]))).toEqual([]));

  const l = computeRows(c.pages, [...A, ...A], [...B, ...B], 1);
  const px = (i: number) => stopPx(stops[i]!, l.tops, 1);
  it("siguiente y anterior se mueven respecto de la marca de lectura", () => {
    expect(stepStop(stops, l.tops, 1, -1e9, 1)).toBe(0);
    expect(stepStop(stops, l.tops, 1, px(0), 1)).toBe(1);
    expect(stepStop(stops, l.tops, 1, px(1), -1)).toBe(0);
    expect(stepStop(stops, l.tops, 1, px(0), -1)).toBeNull();
    expect(stepStop(stops, l.tops, 1, px(4), 1)).toBeNull();
  });
  it("el contador dice en qué cambio se está", () => {
    expect(currentStop(stops, l.tops, 1, 0)).toBeNull();
    expect(currentStop(stops, l.tops, 1, px(1))).toBe(1);
    expect(currentStop(stops, l.tops, 1, px(3) + 5)).toBe(3);
  });
});

describe("rectángulos", () => {
  it("se escalan con el zoom y llevan un pequeño margen", () => {
    expect(rectStyle({ x0: 10, y0: 20, x1: 110, y1: 40 }, 2, 1)).toEqual({ left: "19px", top: "39px", width: "202px", height: "42px" });
  });
});
