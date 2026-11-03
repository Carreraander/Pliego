import { describe, expect, it } from "vitest";
import { addMessage, chatOf, clearChat, formatSize, progressFraction, sourcePages, splitCitations } from "./chat";

describe("splitCitations", () => {
  it("separa texto y citas", () => {
    expect(splitCitations("Seis meses [p. 12] y multa [p. 3].")).toEqual([
      { kind: "text", text: "Seis meses " }, { kind: "page", page: 11 },
      { kind: "text", text: " y multa " }, { kind: "page", page: 2 }, { kind: "text", text: "." },
    ]);
  });
  it("admite listas y rangos", () => {
    const pages = splitCitations("[págs. 3, 5-6]").filter((s) => s.kind === "page").map((s) => (s as { page: number }).page);
    expect(pages).toEqual([2, 4, 5]);
  });
  it("ignora lo que no es una cita y los rangos absurdos", () => {
    expect(splitCitations("sin citas")).toEqual([{ kind: "text", text: "sin citas" }]);
    expect(splitCitations("[p. 0] [p. 1-9999999]").some((s) => s.kind === "page")).toBe(false);
  });
});

describe("utilidades", () => {
  it("formatea tamaños", () => {
    expect(formatSize(820)).toBe("820 MB");
    expect(formatSize(2498, "es")).toBe("2,5 GB");
  });
  it("calcula el progreso", () => {
    expect(progressFraction(null)).toBeNull();
    expect(progressFraction({ done: 5, total: 0 })).toBeNull();
    expect(progressFraction({ done: 50, total: 200 })).toBe(0.25);
    expect(progressFraction({ done: 300, total: 200 })).toBe(1);
  });
  it("ordena y deduplica las páginas fuente", () => {
    const c = (page: number) => ({ page, snippet: "", cited_by_model: true });
    expect(sourcePages([c(4), c(1), c(4)])).toEqual([1, 4]);
  });
});

describe("conversación", () => {
  it("guarda mensajes por documento y se puede borrar", () => {
    addMessage("doc-a", { role: "user", text: "hola" });
    addMessage("doc-b", { role: "error", text: "x" });
    expect(chatOf("doc-a")).toHaveLength(1);
    expect(chatOf("doc-b")).toHaveLength(1);
    clearChat("doc-a");
    expect(chatOf("doc-a")).toHaveLength(0);
    expect(chatOf("doc-b")).toHaveLength(1);
  });
});
