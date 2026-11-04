import { describe, expect, it } from "vitest";
import { boxOf, freeName, nameError, newBox, newSpec, parseOptions, patchOf, type FieldInfo } from "./geometry";

const base: FieldInfo = { name: "a", kind: "text", page: 0, rect: [0, 0, 10, 10], required: false, read_only: false, options: [], multiline: false };

describe("geometría del diseñador de formularios", () => {
  it("normaliza el arrastre en cualquier dirección", () => {
    expect(boxOf([50, 80], [10, 20])).toEqual([10, 20, 50, 80]);
  });
  it("un clic crea un campo del tamaño por defecto; un arrastre, el arrastrado", () => {
    expect(newBox("text", [10, 10], [11, 11])).toEqual([10, 10, 150, 32]);
    expect(newBox("check_box", [10, 10], [10, 10])).toEqual([10, 10, 24, 24]);
    expect(newBox("text", [100, 50], [20, 30])).toEqual([20, 30, 100, 50]);
  });
  it("elige el primer nombre libre", () => {
    expect(freeName("text", ["Texto1", "Texto2", "otro"])).toBe("Texto3");
    expect(freeName("signature", [])).toBe("Firma1");
  });
  it("valida nombres: vacío, con punto y repetido", () => {
    expect(nameError("  ", [])).toBe("fdesign.err.nameEmpty");
    expect(nameError("a.b", [])).toBe("fdesign.err.nameDot");
    expect(nameError("x", ["x"])).toBe("fdesign.err.nameTaken");
    expect(nameError("x", ["x"], "x")).toBeNull();
  });
  it("las opciones van una por línea sin vacías ni repetidas", () => {
    expect(parseOptions("Sí\n\n No \r\nSí")).toEqual(["Sí", "No"]);
  });
  it("completa lo que exige cada tipo", () => {
    expect(newSpec("radio_button", 1, [0, 0, 1, 1], "g").export_value).toBe("g");
    expect((newSpec("combo_box", 0, [0, 0, 1, 1], "c").options as string[]).length).toBe(2);
    expect(newSpec("combo_box", 0, [0, 0, 1, 1], "c", { options: ["x"] }).options).toEqual(["x"]);
    expect(newSpec("push_button", 0, [0, 0, 1, 1], "Boton1").caption).toBe("Boton1");
  });
  it("el parche solo lleva lo que cambió", () => {
    const same = { name: "a", required: false, read_only: false, tooltip: "", options: [], value: "" };
    expect(patchOf(base, same)).toEqual({});
    expect(patchOf(base, { ...same, name: "b", required: true, tooltip: "ayuda" })).toEqual({ name: "b", required: true, tooltip: "ayuda" });
    const combo = { ...base, kind: "combo_box" as const, options: ["a", "b"] };
    expect(patchOf(combo, { ...same, options: ["a", "c"] })).toEqual({ options: ["a", "c"] });
  });
});
