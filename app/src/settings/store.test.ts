import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, flushSettings, loadSettings, migrate, resetSettings, setSettingsBackend, settings, updateSettings, type SettingsBackend } from "./store";

let saved: unknown = null;
const memory: SettingsBackend = { load: async () => saved, save: async (v) => { saved = structuredClone(v); } };

beforeEach(() => { saved = null; setSettingsBackend(memory); });

describe("ajustes", () => {
  it("sin datos usa los valores por defecto", async () => {
    expect(await loadSettings()).toEqual(DEFAULT_SETTINGS);
  });
  it("ignora tipos incorrectos y conserva campos desconocidos", () => {
    const s = migrate({ general: { theme: 5, language: "en" }, futuro: { a: 1 }, shortcuts: { "x.y": "Ctrl+Q", malo: 3 } });
    expect(s.general.theme).toBe("auto");
    expect(s.general.language).toBe("en");
    expect(s.shortcuts).toEqual({ "x.y": "Ctrl+Q" });
    expect((s as unknown as Record<string, unknown>).futuro).toEqual({ a: 1 });
  });
  it("el idioma por defecto es español y un «auto» antiguo pasa a español", () => {
    expect(DEFAULT_SETTINGS.general.language).toBe("es");
    expect(migrate({ version: 1, general: { language: "auto" } }).general.language).toBe("es");
    expect(migrate({ version: 2, general: { language: "auto" } }).general.language).toBe("auto");
    expect(migrate({ version: 1, general: { language: "en" } }).general.language).toBe("en");
  });
  it("persiste los cambios al instante y se recuperan al cargar", async () => {
    await loadSettings();
    updateSettings("modes", { pinned: ["optimize.compress"] });
    updateSettings("seenVersion", "0.2.0");
    await flushSettings();
    expect(settings().modes.pinned).toEqual(["optimize.compress"]);
    saved = structuredClone(saved);
    expect((await loadSettings()).seenVersion).toBe("0.2.0");
    resetSettings();
    await flushSettings();
    expect(settings()).toEqual(DEFAULT_SETTINGS);
  });
});
