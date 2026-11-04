import { describe, expect, it } from "vitest";
import { insertArgs, numberArgs } from "./request";

const ins = { path: "a.pdf", total: 10, kind: "blank" as const, count: "2", source: null, pages: "", where: "11", suffix: "insertado" };

describe("insertArgs", () => {
  it("convierte la posición a índice desde 0", () => {
    expect(insertArgs(ins)).toEqual({ path: "a.pdf", at: 10, suffix: "insertado", blank: 2 });
  });
  it("de otro archivo, con páginas elegidas", () => {
    expect(insertArgs({ ...ins, kind: "file", source: "b.pdf", pages: "1, 3", where: "1" })).toEqual({ path: "a.pdf", at: 0, suffix: "insertado", sourcePath: "b.pdf", pages: [0, 2] });
  });
  it("rechaza lo no válido", () => {
    expect(insertArgs({ ...ins, where: "12" })).toBeNull();
    expect(insertArgs({ ...ins, count: "0" })).toBeNull();
    expect(insertArgs({ ...ins, kind: "file" })).toBeNull();
  });
});

describe("numberArgs", () => {
  const num = { path: "a.pdf", total: 5, style: "lower_roman", prefix: "", first: "1", from: "2", suffix: "numerado" };
  it("crea una regla desde la página elegida", () => {
    expect(numberArgs(num)).toEqual({ path: "a.pdf", suffix: "numerado", rules: [{ start_page: 1, style: "lower_roman", prefix: "", first_number: 1 }] });
  });
  it("rechaza páginas fuera del documento", () => {
    expect(numberArgs({ ...num, from: "6" })).toBeNull();
    expect(numberArgs({ ...num, first: "0" })).toBeNull();
  });
});
