import { describe, expect, it } from "vitest";
import { displayTitle, fileName } from "./docTitle";

describe("título de la pestaña", () => {
  it("usa el título del PDF cuando dice algo", () => {
    expect(displayTitle("Contrato de alquiler", "C:\\docs\\a.pdf")).toBe("Contrato de alquiler");
  });
  it("cae al nombre del archivo sin título o con un título de relleno", () => {
    for (const t of [null, undefined, "", "  ", "untitled", "Untitled", "Untitled 2", "Sin título", "sin titulo", "Unbenannt"]) {
      expect(displayTitle(t, "C:\\docs\\informe.pdf")).toBe("informe.pdf");
    }
  });
  it("entiende rutas con / y con \\", () => {
    expect(fileName("C:/Users/ana/Documentos/informe.pdf")).toBe("informe.pdf");
    expect(fileName("C:\\Users\\ana\\informe.pdf")).toBe("informe.pdf");
    expect(displayTitle("untitled", "D:/dev/x/plano.pdf")).toBe("plano.pdf");
  });
  it("no confunde un título real que empieza por «untitled»", () => {
    expect(displayTitle("Untitled works of 1920", "a.pdf")).toBe("Untitled works of 1920");
  });
});
