import { beforeEach, describe, expect, it, vi } from "vitest";

const goToPage = vi.fn();
vi.mock("../../state/navigation", () => ({ goToPage: (...a: unknown[]) => goToPage(...a) }));

import { canGoBack, canGoForward, goBack, goForward, jumpTo, registerPlaceProbe, resetTrails } from "./history";

describe("historia de saltos", () => {
  let now = { page: 0, y: 0 };
  beforeEach(() => {
    goToPage.mockClear();
    resetTrails();
    now = { page: 0, y: 0 };
    registerPlaceProbe(() => now);
  });
  /** Simula que el visor llega adonde le mandan. */
  const arrive = () => { const [page, y] = goToPage.mock.calls.at(-1)!; now = { page, y: y ?? 0 }; };

  it("un salto apunta de dónde se sale y Atrás/Adelante lo recorren", () => {
    jumpTo(1, { page: 4, y: 100 }); arrive();
    jumpTo(1, { page: 9 }); arrive();
    expect(canGoBack(1)).toBe(true);
    expect(goBack(1)).toBe(true); arrive();
    expect(goToPage).toHaveBeenLastCalledWith(4, 100);
    expect(goBack(1)).toBe(true); arrive();
    expect(goToPage).toHaveBeenLastCalledWith(0, 0);
    expect(goBack(1)).toBe(false);
    expect(canGoForward(1)).toBe(true);
    expect(goForward(1)).toBe(true);
    expect(goToPage).toHaveBeenLastCalledWith(4, 100);
  });
  it("un salto nuevo borra el «Adelante»", () => {
    jumpTo(1, { page: 3 }); arrive();
    goBack(1); arrive();
    expect(canGoForward(1)).toBe(true);
    jumpTo(1, { page: 7 });
    expect(canGoForward(1)).toBe(false);
  });
  it("cada documento tiene su historia", () => {
    jumpTo(1, { page: 3 });
    expect(canGoBack(1)).toBe(true);
    expect(canGoBack(2)).toBe(false);
    expect(goBack(2)).toBe(false);
  });
  it("no apunta dos veces el mismo lugar seguido", () => {
    jumpTo(1, { page: 3 }); // sale de la 0
    now = { page: 0, y: 1 };
    jumpTo(1, { page: 5 }); // vuelve a salir (casi) de la 0
    arrive();
    goBack(1); goBack(1);
    expect(canGoBack(1)).toBe(false);
  });
});
