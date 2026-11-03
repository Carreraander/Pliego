import { describe, expect, it } from "vitest";
import en from "./en.json";
import es from "./es.json";
import { detectLocale, translate } from "./index";

describe("i18n", () => {
  it("es.json y en.json tienen las mismas claves", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(es).sort());
  });
  it("sustituye variables", () => {
    expect(translate("es", "status.page", { page: 3, total: 10 })).toBe("Página 3 de 10");
    expect(translate("en", "search.count", { current: 1, total: 2 })).toBe("1 of 2");
  });
  it("detecta el idioma", () => {
    expect(detectLocale("en-US")).toBe("en");
    expect(detectLocale("es-ES")).toBe("es");
    expect(detectLocale(undefined)).toBe("es");
  });
});
