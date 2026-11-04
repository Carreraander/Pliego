import { describe, expect, it } from "vitest";
import { parseRanges } from "./ranges";

describe("parseRanges", () => {
  it("pasa de páginas desde 1 a tramos desde 0", () => {
    expect(parseRanges("1-3, 5, 8–10", 12)).toEqual([[0, 2], [4, 4], [7, 9]]);
  });
  it("rechaza lo vacío, lo mal escrito y lo fuera de rango", () => {
    expect(parseRanges("", 5)).toBeNull();
    expect(parseRanges("a-b", 5)).toBeNull();
    expect(parseRanges("0-2", 5)).toBeNull();
    expect(parseRanges("4-2", 5)).toBeNull();
    expect(parseRanges("3-6", 5)).toBeNull();
  });
});
