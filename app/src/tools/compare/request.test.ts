import { describe, expect, it } from "vitest";
import { compareArgs, sameFile } from "./request";

const base = { path: "C:\\Docs\\v1.pdf", other: "C:\\Docs\\v2.pdf", text: true, visual: true, label: "comparación" };

describe("Comparar", () => {
  it("arma los argumentos con los nombres que espera el comando", () => {
    expect(compareArgs(base)).toEqual({ path: "C:\\Docs\\v1.pdf", other: "C:\\Docs\\v2.pdf", passwordA: null, passwordB: null, text: true, visual: true, label: "comparación" });
  });
  it("no deja comparar sin segundo documento, con el mismo archivo o sin nada que comparar", () => {
    expect(compareArgs({ ...base, other: null })).toBeNull();
    expect(compareArgs({ ...base, other: "c:/docs/V1.pdf" })).toBeNull();
    expect(compareArgs({ ...base, text: false, visual: false })).toBeNull();
    expect(compareArgs({ ...base, path: "" })).toBeNull();
  });
  it("compara rutas de Windows sin distinguir mayúsculas ni barras", () => {
    expect(sameFile("C:\\A\\b.pdf", "c:/a/B.pdf")).toBe(true);
    expect(sameFile("C:\\A\\b.pdf", "C:\\A\\c.pdf")).toBe(false);
  });
});
