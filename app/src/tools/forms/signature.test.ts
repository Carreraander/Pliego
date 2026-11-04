import { describe, expect, it } from "vitest";
import { normalize, placeSignature } from "./signature";

describe("Mi firma", () => {
  it("normaliza a su caja y conserva la proporción", () => {
    const sig = normalize([[[10, 20], [110, 20], [110, 70]]])!;
    expect(sig.aspect).toBe(2);
    expect(sig.strokes[0]![0]).toEqual([0, 0]);
    expect(sig.strokes[0]![2]).toEqual([1, 1]);
  });
  it("no guarda una firma vacía", () => {
    expect(normalize([])).toBeNull();
    expect(normalize([[[5, 5], [5.5, 5.5]]])).toBeNull();
  });
  it("la coloca centrada en el punto con el ancho pedido", () => {
    const sig = normalize([[[0, 0], [100, 50]]])!;
    const out = placeSignature(sig, [200, 100], 100);
    expect(out[0]![0]).toEqual([150, 75]);
    expect(out[0]![1]).toEqual([250, 125]);
  });
});
