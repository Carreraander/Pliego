import { describe, expect, it } from "vitest";
import { bestScore, fuzzyScore, normalize } from "./fuzzy";

describe("fuzzy", () => {
  it("ignora tildes y mayúsculas", () => {
    expect(normalize("Índice")).toBe("indice");
    expect(fuzzyScore("INDICE", "Índice de búsqueda")).not.toBeNull();
    expect(fuzzyScore("busqueda", "Índice de búsqueda")).not.toBeNull();
  });
  it("prefiere prefijos de palabra y el inicio", () => {
    const prefix = fuzzyScore("com", "Comprimir")!;
    const inner = fuzzyScore("com", "Descomprimir")!;
    expect(prefix).toBeGreaterThan(inner);
  });
  it("acepta subsecuencias y descarta lo que no coincide", () => {
    expect(fuzzyScore("cmprm", "Comprimir")).not.toBeNull();
    expect(fuzzyScore("zzz", "Comprimir")).toBeNull();
  });
  it("exige todas las palabras", () => {
    expect(fuzzyScore("marca agua", "Marca de agua")).not.toBeNull();
    expect(fuzzyScore("marca fuego", "Marca de agua")).toBeNull();
  });
  it("las descripciones no aceptan subsecuencias", () => {
    expect(bestScore("mrcgu", [], ["Añade una marca de agua"])).toBeNull();
    expect(bestScore("marca", [], ["Añade una marca de agua"])).not.toBeNull();
  });
  it("consulta vacía coincide con todo y bestScore toma el mejor texto", () => {
    expect(fuzzyScore("", "x")).toBe(0);
    expect(bestScore("reducir", ["Comprimir", "reducir", "tamaño"])).not.toBeNull();
    expect(bestScore("nada", ["Comprimir"])).toBeNull();
  });
});
