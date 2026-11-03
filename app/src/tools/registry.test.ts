import { beforeEach, describe, expect, it } from "vitest";
import { setLocale } from "../i18n";
import { registerBaseTools } from "./base";
import { getTool, NO_DOCUMENT, registerTool, setDocFacts, TOOL_GROUPS, toolState, tools, toolsByGroup } from "./registry";

beforeEach(() => { setLocale("es"); setDocFacts(NO_DOCUMENT); registerBaseTools(); });

describe("registro de herramientas", () => {
  it("el mapa completo de §4.3 está registrado: todos los grupos tienen herramientas", () => {
    expect(toolsByGroup().map((g) => g.group)).toEqual([...TOOL_GROUPS]);
    expect(tools().length).toBeGreaterThanOrEqual(45);
  });
  it("los ids son únicos y registrar de nuevo sustituye a la fantasma", () => {
    const ids = tools().map((t) => t.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(getTool("optimize.compress")?.ghost).toBe(true);
    registerTool({ id: "optimize.compress", group: "compare-optimize", kind: "task", title: "t", description: "d", icon: "file-down", when: { document: true } });
    expect(getTool("optimize.compress")?.ghost).toBeUndefined();
    expect(tools().filter((t) => t.id === "optimize.compress")).toHaveLength(1);
  });
  it("evalúa las condiciones contra el estado del documento", () => {
    const sign = getTool("sign.validate")!;
    registerTool({ ...sign, ghost: false });
    expect(toolState(getTool("sign.validate")!)).toEqual({ enabled: false, reason: "Abre un documento para usarla" });
    setDocFacts({ document: true });
    expect(toolState(getTool("sign.validate")!)).toEqual({ enabled: false, reason: "Este documento no tiene firmas" });
    setDocFacts({ signatures: true });
    expect(toolState(getTool("sign.validate")!).enabled).toBe(true);
  });
  it("las fantasma dicen que llegarán más adelante", () => {
    expect(toolState(getTool("ocr.recognize")!)).toEqual({ enabled: false, reason: "Disponible más adelante" });
  });
  it("Leer es real y apunta al modo del visor", () => {
    const read = getTool("viewer.read")!;
    expect(read.ghost).toBeUndefined();
    expect(read.modeId).toBe("read");
    expect(toolState(read).enabled).toBe(true);
  });
});
