import { describe, expect, it } from "vitest";
import { clearSelection, deleteSelected, duplicateSelected, initialEdits, isDirty, movePage, moveSelected, rotateSelected, select, selectAll, selectionEdits, toEdits } from "./pageedits";

const pagesOf = (e: ReturnType<typeof initialEdits>) => e.items.map((i) => i.page);

describe("selección", () => {
  it("clic, Ctrl y Mayús", () => {
    let e = initialEdits(6);
    e = select(e, 1, "single");
    e = select(e, 4, "toggle");
    expect(e.selected).toEqual([1, 4]);
    e = select(e, 2, "toggle");
    e = select(e, 2, "toggle");
    expect(e.selected).toEqual([1, 4]);
    e = select(clearSelection(e), 1, "single");
    e = select(e, 3, "range");
    expect(e.selected).toEqual([1, 2, 3]);
    expect(selectAll(e).selected).toHaveLength(6);
  });
});

describe("edición", () => {
  it("gira solo las seleccionadas y da la vuelta a 360", () => {
    let e = select(initialEdits(3), 1, "single");
    e = rotateSelected(rotateSelected(e, 90), 270);
    expect(e.items.map((i) => i.rotate)).toEqual([0, 0, 0]);
    e = rotateSelected(e, -90);
    expect(e.items.map((i) => i.rotate)).toEqual([0, 270, 0]);
  });
  it("borra pero nunca deja el documento vacío", () => {
    let e = select(initialEdits(3), 0, "single");
    e = deleteSelected(e);
    expect(pagesOf(e)).toEqual([1, 2]);
    expect(pagesOf(deleteSelected(selectAll(e)))).toEqual([1, 2]);
  });
  it("duplica después de cada seleccionada con claves nuevas", () => {
    const e = duplicateSelected(select(initialEdits(3), 1, "single"));
    expect(pagesOf(e)).toEqual([0, 1, 1, 2]);
    expect(new Set(e.items.map((i) => i.key)).size).toBe(4);
    expect(e.selected).toEqual([2]);
  });
  it("mueve la selección y respeta los bordes", () => {
    let e = select(initialEdits(4), 1, "single");
    e = moveSelected(e, 1);
    expect(pagesOf(e)).toEqual([0, 2, 1, 3]);
    expect(e.selected).toEqual([2]);
    e = moveSelected(select(initialEdits(3), 0, "single"), -1);
    expect(pagesOf(e)).toEqual([0, 1, 2]);
    let m = select(select(initialEdits(4), 2, "single"), 3, "toggle");
    m = moveSelected(m, 1);
    expect(pagesOf(m)).toEqual([0, 1, 2, 3]);
    m = moveSelected(m, -1);
    expect(pagesOf(m)).toEqual([0, 2, 3, 1]);
    expect(m.selected).toEqual([1, 2]);
  });
  it("arrastrar mueve una página", () => {
    expect(pagesOf(movePage(initialEdits(4), 0, 2))).toEqual([1, 2, 0, 3]);
  });
});

describe("resultado", () => {
  it("toEdits y selectionEdits", () => {
    let e = rotateSelected(select(initialEdits(3), 2, "single"), 90);
    expect(toEdits(e)).toEqual([{ page: 0, rotate: 0 }, { page: 1, rotate: 0 }, { page: 2, rotate: 90 }]);
    expect(selectionEdits(e)).toEqual([{ page: 2, rotate: 90 }]);
    e = select(e, 0, "toggle");
    expect(selectionEdits(e).map((x) => x.page)).toEqual([0, 2]);
  });
  it("isDirty", () => {
    const e = initialEdits(3);
    expect(isDirty(e, 3)).toBe(false);
    expect(isDirty(rotateSelected(select(e, 0, "single"), 90), 3)).toBe(true);
    expect(isDirty(movePage(e, 0, 1), 3)).toBe(true);
  });
});
