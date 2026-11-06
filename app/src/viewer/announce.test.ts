import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { announce, announcement, brief, resetAnnouncements } from "./announce";

describe("anuncios para lectores de pantalla", () => {
  beforeEach(() => { vi.useFakeTimers(); resetAnnouncements(); });
  afterEach(() => vi.useRealTimers());

  it("escribe el mensaje y lo retira solo", () => {
    announce("Página 3 de 10");
    expect(announcement()).toBe("");
    vi.advanceTimersByTime(40);
    expect(announcement()).toBe("Página 3 de 10");
    vi.advanceTimersByTime(4100);
    expect(announcement()).toBe("");
  });
  it("repetir el mismo texto lo vuelve a poner", () => {
    announce("hola");
    vi.advanceTimersByTime(40);
    announce("hola");
    expect(announcement()).toBe("");
    vi.advanceTimersByTime(40);
    expect(announcement()).toBe("hola");
  });
  it("solo vale el último de varios seguidos", () => {
    announce("uno");
    announce("dos");
    vi.advanceTimersByTime(40);
    expect(announcement()).toBe("dos");
  });
  it("recorta lo largo y junta espacios", () => {
    expect(brief("  a \n  b  ")).toBe("a b");
    expect(brief("x".repeat(500), 20)).toHaveLength(20);
  });
});
