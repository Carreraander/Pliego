import { beforeEach, describe, expect, it } from "vitest";
import { activeToasts, clearToasts, toast } from "./toastStore";

beforeEach(() => clearToasts());

describe("avisos", () => {
  it("el mismo aviso repetido no se apila", () => {
    for (let i = 0; i < 4; i++) toast({ tone: "danger", text: "error de E/S" });
    expect(activeToasts()).toHaveLength(1);
  });
  it("avisos distintos o de otro tono sí conviven", () => {
    toast({ tone: "danger", text: "a" });
    toast({ tone: "info", text: "a" });
    toast({ tone: "danger", text: "b" });
    expect(activeToasts().map((t) => t.text)).toEqual(["a", "a", "b"]);
  });
});
