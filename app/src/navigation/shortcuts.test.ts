import { beforeEach, describe, expect, it } from "vitest";
import { loadSettings, setSettingsBackend } from "../settings/store";
import { displayCombo, eventCombo, normalizeCombo } from "./keys";
import { assignShortcut, conflicts, effectiveKeys, matchShortcut, registerShortcut, resetShortcut, resetShortcuts, shortcutDefs } from "./shortcuts";

const ev = (key: string, mods: Partial<{ ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }> = {}) =>
  ({ key, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

beforeEach(async () => {
  setSettingsBackend({ load: async () => null, save: async () => undefined });
  await loadSettings();
  resetShortcuts();
});

describe("combinaciones", () => {
  it("normaliza nombres en español y orden de modificadores", () => {
    expect(normalizeCombo("mayús+ctrl+a")).toBe("Ctrl+Shift+A");
    expect(normalizeCombo("Supr")).toBe("Delete");
    expect(normalizeCombo("f11")).toBe("F11");
    expect(normalizeCombo("Ctrl++")).toBe("Ctrl++");
  });
  it("lee eventos y muestra en español", () => {
    expect(eventCombo(ev("a", { ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+A");
    expect(eventCombo(ev("Shift", { shiftKey: true }))).toBeNull();
    expect(displayCombo("Ctrl+Shift+A", "es")).toBe("Ctrl+Mayús+A");
    expect(displayCombo("Ctrl+Shift+A", "en")).toBe("Ctrl+Shift+A");
  });
});

describe("gestor", () => {
  it("ejecuta la orden y respeta las letras sueltas dentro de campos", () => {
    registerShortcut({ id: "t.highlight", title: "Resaltar", keys: "H", scope: "canvas", section: "tools", run: () => undefined });
    registerShortcut({ id: "t.open", title: "Abrir", keys: "Ctrl+O", section: "file", run: () => undefined });
    expect(matchShortcut(ev("h"), false)?.id).toBe("t.highlight");
    expect(matchShortcut(ev("h"), true)).toBeNull();
    expect(matchShortcut(ev("o", { ctrlKey: true }), true)?.id).toBe("t.open");
  });
  it("no reasigna una combinación ya usada y detecta conflictos", () => {
    registerShortcut({ id: "a", title: "Orden A", keys: "Ctrl+1", section: "zoom", run: () => undefined });
    registerShortcut({ id: "b", title: "Orden B", keys: "Ctrl+2", section: "zoom", run: () => undefined });
    expect(assignShortcut("b", "ctrl+1")).toBe("Orden A");
    expect(effectiveKeys(shortcutDefs()[1]!)).toBe("Ctrl+2");
    expect(conflicts()).toEqual([]);
    expect(assignShortcut("b", "Ctrl+3")).toBeNull();
    expect(matchShortcut(ev("3", { ctrlKey: true }), false)?.id).toBe("b");
    resetShortcut("b");
    expect(matchShortcut(ev("2", { ctrlKey: true }), false)?.id).toBe("b");
  });
});
