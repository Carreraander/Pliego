import { beforeEach, describe, expect, it } from "vitest";
import { setLocale } from "../i18n";
import { formatOpened, greeting } from "./format";
import { memoryBackend, addRecent, pinRecent, recents, refreshRecents, removeRecent, setRecentsBackend } from "./recents";

beforeEach(() => { setLocale("es"); setRecentsBackend(memoryBackend()); });

describe("Inicio", () => {
  it("el saludo depende de la hora", () => {
    expect(greeting(8)).toBe("home.morning");
    expect(greeting(15)).toBe("home.afternoon");
    expect(greeting(22)).toBe("home.evening");
  });
  it("fechas: hoy con hora, ayer y fecha corta", () => {
    const now = new Date(2026, 10, 3, 12, 0, 0);
    expect(formatOpened(new Date(2026, 10, 3, 10, 42).getTime() / 1000, now, "es-ES")).toBe("hoy, 10:42");
    expect(formatOpened(new Date(2026, 10, 2, 23, 0).getTime() / 1000, now, "es-ES")).toBe("ayer");
    expect(formatOpened(new Date(2026, 9, 12).getTime() / 1000, now, "es-ES")).toMatch(/12 oct/);
  });
  it("los recientes se anotan, se fijan arriba y se quitan", async () => {
    await addRecent("C:\\a.pdf");
    await addRecent("C:\\b.pdf");
    await pinRecent("C:\\a.pdf", true);
    await refreshRecents();
    expect(recents().map((r) => r.name)).toEqual(["a.pdf", "b.pdf"]);
    expect(recents()[0]!.pinned).toBe(true);
    await removeRecent("C:\\a.pdf");
    expect(recents().map((r) => r.name)).toEqual(["b.pdf"]);
  });
});
