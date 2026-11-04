import { describe, expect, it } from "vitest";
import { COLORS, quadOf, rectOf, specFor, thin } from "./geometry";

const c = COLORS[0]!.rgb;

describe("geometría de comentarios", () => {
  it("normaliza el rectángulo arrastrado en cualquier dirección", () => {
    expect(rectOf([50, 80], [10, 20])).toEqual({ x0: 10, y0: 20, x1: 50, y1: 80 });
  });
  it("el cuadrilátero va en orden ul, ur, ll, lr", () => {
    expect(quadOf({ x0: 1, y0: 2, x1: 3, y1: 4 })).toEqual([1, 2, 3, 2, 1, 4, 3, 4]);
  });
  it("resaltar crea un cuadrilátero con transparencia; un clic suelto no crea nada", () => {
    const s = specFor("highlight", 2, c, { points: [[10, 10], [100, 24]] })!;
    expect(s.page).toBe(2);
    expect(s.geometry).toEqual({ type: "highlight", quads: [[10, 10, 100, 10, 10, 24, 100, 24]] });
    expect((s.style as { opacity: number }).opacity).toBe(0.4);
    expect(specFor("highlight", 0, c, { points: [[10, 10], [11, 11]] })).toBeNull();
  });
  it("tachar usa el nombre strike_out que entiende pliego-annot", () => {
    expect((specFor("strike", 0, c, { points: [[0, 0], [50, 10]] })!.geometry as { type: string }).type).toBe("strike_out");
  });
  it("la nota lleva su texto y el dibujo necesita al menos dos puntos", () => {
    expect(specFor("note", 0, c, { points: [[5, 6]] }, "hola")).toMatchObject({ geometry: { type: "note", at: [5, 6] }, contents: "hola" });
    expect(specFor("ink", 0, c, { points: [[1, 1]] })).toBeNull();
    expect(specFor("ink", 0, c, { points: [[1, 1], [9, 9]] })!.geometry).toEqual({ type: "ink", strokes: [[[1, 1], [9, 9]]] });
  });
  it("adelgaza el trazo pero conserva el último punto", () => {
    const out = thin([[0, 0], [0.5, 0], [1, 0], [5, 0], [5.2, 0]]);
    expect(out[0]).toEqual([0, 0]);
    expect(out[out.length - 1]).toEqual([5.2, 0]);
    expect(out.length).toBeLessThan(5);
  });
});
