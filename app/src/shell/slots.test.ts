import { beforeEach, describe, expect, it } from "vitest";
import {
  closeInspector, inspector, noticeFor, openInspector, panels, pickNotice, registerPanel, registerStatusItem, resetSlots, showNotice,
  statusItems,
} from "./slots";

const C = () => null;
beforeEach(() => resetSlots());

describe("registerPanel", () => {
  it("ordena por `order` y se puede retirar", () => {
    const a = registerPanel({ id: "t.a", title: "A", icon: "info", order: 20, component: C });
    registerPanel({ id: "t.b", title: "B", icon: "info", order: 10, component: C });
    expect(panels().filter((p) => p.id.startsWith("t.")).map((p) => p.id)).toEqual(["t.b", "t.a"]);
    a();
    expect(panels().some((p) => p.id === "t.a")).toBe(false);
  });
  it("registrar dos veces el mismo id reemplaza", () => {
    registerPanel({ id: "t.dup", title: "1", icon: "info", component: C });
    registerPanel({ id: "t.dup", title: "2", icon: "info", component: C });
    expect(panels().filter((p) => p.id === "t.dup").map((p) => p.title)).toEqual(["2"]);
  });
});

describe("inspector", () => {
  it("abre y cierra", () => {
    openInspector(C, { x: 1 });
    expect(inspector()?.props).toEqual({ x: 1 });
    closeInspector();
    expect(inspector()).toBeNull();
  });
});

describe("avisos del documento", () => {
  it("muestra el de mayor prioridad: reparado > firmado > protegido", () => {
    showNotice("d1", { kind: "protected", text: "protegido" });
    showNotice("d1", { kind: "repaired", text: "reparado" });
    showNotice("d1", { kind: "signed", text: "firmado" });
    expect(noticeFor("d1")?.text).toBe("reparado");
  });
  it("a igual prioridad gana el más reciente; con el mismo id se reemplaza", () => {
    showNotice("d2", { kind: "form", id: "f", text: "uno" });
    showNotice("d2", { kind: "form", id: "g", text: "dos" });
    expect(noticeFor("d2")?.text).toBe("dos");
    showNotice("d2", { kind: "form", id: "g", text: "tres" });
    expect(noticeFor("d2")?.text).toBe("tres");
  });
  it("quitar el aviso deja el siguiente; sin documento no hay aviso", () => {
    const off = showNotice("d3", { kind: "signed", text: "firmado" });
    showNotice("d3", { kind: "pdfa", text: "pdfa" });
    off();
    expect(noticeFor("d3")?.text).toBe("pdfa");
    expect(noticeFor(null)).toBeNull();
  });
  it("pickNotice de lista vacía es null", () => expect(pickNotice([])).toBeNull());
});

describe("barra de estado", () => {
  it("filtra por ranura y ordena", () => {
    registerStatusItem({ id: "s.b", slot: "end", order: 2, text: () => "b" });
    registerStatusItem({ id: "s.a", slot: "end", order: 1, text: () => "a" });
    registerStatusItem({ id: "s.c", slot: "start", text: () => "c" });
    expect(statusItems("end").filter((i) => i.id.startsWith("s.")).map((i) => i.id)).toEqual(["s.a", "s.b"]);
  });
});
