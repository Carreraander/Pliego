import { describe, expect, it } from "vitest";
import { insertArgs, mergeOptions, NUMBER_TEMPLATE, numberArgs } from "./request";

const ins = { path: "a.pdf", total: 10, kind: "blank" as const, count: "2", source: null, pages: "", where: "11", suffix: "insertado" };

describe("insertArgs", () => {
  it("convierte la posición a índice desde 0", () => {
    expect(insertArgs(ins)).toEqual({ path: "a.pdf", at: 10, suffix: "insertado", blank: 2 });
  });
  it("de otro archivo, con páginas elegidas", () => {
    expect(insertArgs({ ...ins, kind: "file", source: "b.pdf", pages: "1, 3", where: "1" })).toEqual({ path: "a.pdf", at: 0, suffix: "insertado", sourcePath: "b.pdf", sourcePassword: null, pages: [0, 2] });
    expect(insertArgs({ ...ins, kind: "file", source: "b.pdf", sourcePassword: "abre", pages: "1", where: "1" })?.sourcePassword).toBe("abre");
  });
  it("rechaza lo no válido", () => {
    expect(insertArgs({ ...ins, where: "12" })).toBeNull();
    expect(insertArgs({ ...ins, count: "0" })).toBeNull();
    expect(insertArgs({ ...ins, kind: "file" })).toBeNull();
  });
});

describe("numberArgs", () => {
  const num = { path: "a.pdf", total: 5, position: "footer_center", format: "{page} de {pages}", from: "2", size: "10", alsoLabels: false, suffix: "numerado" };
  it("crea la petición con la primera página numerada desde 0", () => {
    expect(numberArgs(num)).toEqual({ path: "a.pdf", suffix: "numerado", request: { position: "footer_center", format: "{page} de {pages}", from_page: 1, size: 10, also_labels: false } });
  });
  it("rechaza páginas fuera del documento, tamaños absurdos y plantillas sin número", () => {
    expect(numberArgs({ ...num, from: "6" })).toBeNull();
    expect(numberArgs({ ...num, size: "2" })).toBeNull();
    expect(numberArgs({ ...num, format: "hola" })).toBeNull();
  });
  it("las plantillas de formato llevan {page}", () => {
    for (const f of Object.values(NUMBER_TEMPLATE)) expect(f("Página", "de")).toContain("{page}");
  });
});

describe("mergeOptions", () => {
  it("lleva la contraseña de cada archivo protegido de la lista y no otras", () => {
    const o = mergeOptions(["a.pdf", "b.pdf"], true, { "a.pdf": "uno", "viejo.pdf": "nada" });
    expect(o).toEqual({ keep_bookmarks: true, bookmark_per_file: true, file_passwords: { "a.pdf": "uno" } });
  });
  it("sin contraseñas deja el mapa vacío", () => {
    expect(mergeOptions(["a.pdf"], false, {})).toEqual({ keep_bookmarks: true, bookmark_per_file: false, file_passwords: {} });
  });
});
