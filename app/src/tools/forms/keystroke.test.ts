import { describe, expect, it } from "vitest";
import { applyKeystroke, describeEdit } from "./keystroke";

describe("edición de un campo de texto", () => {
  it("deduce inserciones, borrados y reemplazos", () => {
    expect(describeEdit("ab", "ab")).toBeNull();
    expect(describeEdit("ab", "abc")).toEqual({ start: 2, end: 2, change: "c" });
    expect(describeEdit("abc", "ac")).toEqual({ start: 1, end: 2, change: "" });
    expect(describeEdit("abcd", "aXd")).toEqual({ start: 1, end: 3, change: "X" });
    expect(describeEdit("", "hola")).toEqual({ start: 0, end: 0, change: "hola" });
  });

  it("cuenta caracteres, no unidades UTF-16", () => {
    expect(describeEdit("a😀", "a😀b")).toEqual({ start: 2, end: 2, change: "b" });
  });

  it("revierte lo rechazado y aplica el cambio del script", () => {
    const edit = { start: 2, end: 2, change: "x" };
    expect(applyKeystroke("ab", edit, { accepted: false, change: "x", messages: [] })).toEqual({ value: "ab", caret: 2 });
    expect(applyKeystroke("ab", edit, { accepted: true, change: "X", messages: [] })).toEqual({ value: "abX", caret: 3 });
    expect(applyKeystroke("abcd", { start: 1, end: 3, change: "z" }, { accepted: true, change: "", messages: [] })).toEqual({ value: "ad", caret: 1 });
  });
});
