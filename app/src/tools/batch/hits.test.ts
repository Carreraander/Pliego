import { describe, expect, it } from "vitest";
import { snippetParts } from "./hits";

describe("fragmentos del índice", () => {
  it("marca las coincidencias", () => {
    expect(snippetParts("hola mundo", [[5, 10]])).toEqual([{ text: "hola ", mark: false }, { text: "mundo", mark: true }]);
  });
  it("los rangos son bytes: la ñ ocupa dos", () => {
    expect(snippetParts("año nuevo", [[4, 5]])).toEqual([{ text: "año", mark: false }, { text: " ", mark: true }, { text: "nuevo", mark: false }]);
  });
  it("sin coincidencias devuelve el texto entero y vacío no devuelve nada", () => {
    expect(snippetParts("abc", [])).toEqual([{ text: "abc", mark: false }]);
    expect(snippetParts("", [])).toEqual([]);
  });
});
