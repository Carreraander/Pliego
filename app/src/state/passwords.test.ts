import { afterEach, describe, expect, it } from "vitest";
import { forgetPassword, passwordFor, rememberPassword, withPassword } from "./passwords";

describe("contraseñas de documentos abiertos", () => {
  afterEach(() => forgetPassword("C:\\docs\\a.pdf"));

  it("se recuerdan por ruta y se olvidan", () => {
    expect(passwordFor("C:\\docs\\a.pdf")).toBeNull();
    rememberPassword("C:\\docs\\a.pdf", "abre");
    expect(passwordFor("C:\\docs\\a.pdf")).toBe("abre");
    forgetPassword("C:\\docs\\a.pdf");
    expect(passwordFor("C:\\docs\\a.pdf")).toBeNull();
  });

  it("una contraseña vacía no se guarda", () => {
    rememberPassword("C:\\docs\\a.pdf", "");
    expect(passwordFor("C:\\docs\\a.pdf")).toBeNull();
  });

  it("completa { path } y { request: { path } } sin pisar la que ya venga", () => {
    rememberPassword("C:\\docs\\a.pdf", "abre");
    expect(withPassword({ path: "C:\\docs\\a.pdf", suffix: "x" })).toEqual({ path: "C:\\docs\\a.pdf", suffix: "x", password: "abre" });
    expect(withPassword({ path: "C:\\docs\\a.pdf", password: "otra" }).password).toBe("otra");
    expect(withPassword({ request: { path: "C:\\docs\\a.pdf", preset: "ebook", password: null } })).toEqual({
      request: { path: "C:\\docs\\a.pdf", preset: "ebook", password: "abre" },
    });
  });

  it("en Comparar rellena la contraseña del primer archivo y no inventa la del otro", () => {
    rememberPassword("C:\\docs\\a.pdf", "abre");
    const a = withPassword({ path: "C:\\docs\\a.pdf", other: "C:\\docs\\b.pdf", passwordA: null, passwordB: null });
    expect(a.passwordA).toBe("abre");
    expect(a.passwordB).toBeNull();
  });

  it("completa los comandos de edición sobre el propio PDF (comentar, rellenar, diseñar, medir, editar texto)", () => {
    rememberPassword("C:\\docs\\a.pdf", "abre");
    for (const extra of [{ spec: {} }, { values: {} }, { name: "campo", patch: {} }, { page: 0, scale: {}, points: [] }, { page: 0, block: 1, text: "x" }]) {
      expect(withPassword({ docId: 1, path: "C:\\docs\\a.pdf", ...extra }).password).toBe("abre");
    }
  });

  it("no añade claves ajenas a comandos que no las tienen", () => {
    rememberPassword("C:\\docs\\a.pdf", "abre");
    expect("passwordA" in withPassword({ path: "C:\\docs\\a.pdf" })).toBe(false);
  });

  it("sin contraseña recordada deja los argumentos como están y no los modifica", () => {
    const args = { path: "C:\\docs\\a.pdf", password: null };
    expect(withPassword(args)).toEqual(args);
    rememberPassword("C:\\docs\\a.pdf", "abre");
    withPassword(args);
    expect(args.password).toBeNull();
  });
});
