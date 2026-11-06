import { describe, expect, it } from "vitest";
import { isBrowserShortcut } from "./browserGuard";

const ev = (key: string, mods: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean } = {}) =>
  ({ key, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods });

describe("isBrowserShortcut", () => {
  it("para recargar, imprimir, buscar y navegar del navegador", () => {
    for (const e of [ev("F5"), ev("r", { ctrlKey: true }), ev("R", { ctrlKey: true, shiftKey: true }), ev("F5", { ctrlKey: true }),
      ev("p", { ctrlKey: true }), ev("f", { ctrlKey: true }), ev("F3"), ev("F3", { shiftKey: true }), ev("ArrowLeft", { altKey: true }),
      ev("n", { ctrlKey: true }), ev("s", { ctrlKey: true }), ev("u", { ctrlKey: true }), ev("F7"), ev("BrowserBack"),
      ev("=", { ctrlKey: true }), ev("-", { ctrlKey: true }), ev("0", { ctrlKey: true }), ev("I", { ctrlKey: true, shiftKey: true })]) {
      expect(isBrowserShortcut(e), JSON.stringify(e)).toBe(true);
    }
  });

  it("deja la edición de texto y las teclas normales", () => {
    for (const e of [ev("a", { ctrlKey: true }), ev("c", { ctrlKey: true }), ev("v", { ctrlKey: true }), ev("x", { ctrlKey: true }),
      ev("z", { ctrlKey: true }), ev("y", { ctrlKey: true }), ev("Z", { ctrlKey: true, shiftKey: true }), ev("a"), ev("Enter"),
      ev("ArrowLeft"), ev("Home"), ev("F1"), ev("F4"), ev("F6"), ev("F9"), ev("F11"), ev("1", { altKey: true }),
      ev("2", { ctrlKey: true, altKey: true })]) {
      expect(isBrowserShortcut(e), JSON.stringify(e)).toBe(false);
    }
  });

  it("en desarrollo deja abrir las herramientas de desarrollo", () => {
    expect(isBrowserShortcut(ev("F12"), true)).toBe(false);
    expect(isBrowserShortcut(ev("I", { ctrlKey: true, shiftKey: true }), true)).toBe(false);
    expect(isBrowserShortcut(ev("F12"), false)).toBe(true);
    expect(isBrowserShortcut(ev("F5"), true)).toBe(true);
  });
});
