import { describe, expect, it } from "vitest";

// `present.ts` importa el estado de la app, que necesita el DOM al cargarse; aquí solo se prueba la tabla de teclas.
import { vi } from "vitest";
vi.mock("../../shell/platform", () => ({ tauriWindow: () => null }));
vi.mock("../../state/documents", () => ({ activeDoc: () => null, currentPageIndex: () => 0, rotation: () => 0, setRotation: () => {}, setZoom: () => {}, zoom: () => 1 }));
vi.mock("../../state/navigation", () => ({ goToPage: () => {} }));
vi.mock("../../viewer/layout", () => ({ fitPageZoom: () => 1 }));

const { targetPage } = await import("./present");

describe("presentación: teclas", () => {
  it("avanza y retrocede una página", () => {
    expect(targetPage("ArrowRight", 2, 10)).toBe(3);
    expect(targetPage(" ", 2, 10)).toBe(3);
    expect(targetPage("PageUp", 2, 10)).toBe(1);
    expect(targetPage("ArrowLeft", 2, 10)).toBe(1);
  });
  it("no pasa de los extremos", () => {
    expect(targetPage("ArrowLeft", 0, 10)).toBeNull();
    expect(targetPage("ArrowRight", 9, 10)).toBeNull();
    expect(targetPage("Home", 0, 10)).toBeNull();
    expect(targetPage("End", 3, 10)).toBe(9);
    expect(targetPage("Home", 3, 10)).toBe(0);
  });
  it("ignora otras teclas y documentos vacíos", () => {
    expect(targetPage("a", 2, 10)).toBeNull();
    expect(targetPage("ArrowRight", 0, 0)).toBeNull();
  });
});
