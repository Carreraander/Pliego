import { describe, expect, it } from "vitest";
import { addNode, deleteNode, indentNode, moveNode, nodeAt, outdentNode, renameNode, retargetNode, type OutlineNode } from "./outlineEdit";

const n = (title: string, page: number | null, children: OutlineNode[] = []): OutlineNode => ({ title, page, children });
const tree = (): OutlineNode[] => [n("A", 0, [n("A1", 1), n("A2", 2)]), n("B", 3), { ...n("Web", null), uri: "https://x" }];
const titles = (nodes: OutlineNode[]): unknown => nodes.map((x) => (x.children.length ? [x.title, titles(x.children)] : x.title));

describe("edición de marcadores", () => {
  it("añade detrás de la selección o al final", () => {
    const r = addNode(tree(), [0, 0], "Nuevo", 5);
    expect(titles(r.nodes)).toEqual([["A", ["A1", "Nuevo", "A2"]], "B", "Web"]);
    expect(r.path).toEqual([0, 1]);
    expect(nodeAt(r.nodes, r.path)).toMatchObject({ page: 5 });
    expect(addNode([], null, "Primero", 0)).toEqual({ nodes: [n("Primero", 0)], path: [0] });
  });

  it("renombra sin aceptar nombres vacíos y sin tocar el original", () => {
    const t = tree();
    expect(nodeAt(renameNode(t, [1], "  Capítulo B ")!.nodes, [1])!.title).toBe("Capítulo B");
    expect(renameNode(t, [1], "   ")).toBeNull();
    expect(t[1]!.title).toBe("B");
  });

  it("cambiar de página quita la altura y el enlace web", () => {
    const t = tree();
    t[2]!.top = 400;
    expect(nodeAt(retargetNode(t, [2], 7)!.nodes, [2])).toMatchObject({ page: 7, top: null, uri: null });
  });

  it("borra con sus hijos y selecciona lo siguiente", () => {
    const r = deleteNode(tree(), [0])!;
    expect(titles(r.nodes)).toEqual(["B", "Web"]);
    expect(r.path).toEqual([0]);
    expect(deleteNode(tree(), [0, 1])!.path).toEqual([0, 0]);
    expect(deleteNode([n("Solo", 0, [n("Hijo", 1)])], [0, 0])!.path).toEqual([0]);
  });

  it("sube, baja, sangra y quita la sangría", () => {
    expect(moveNode(tree(), [0], -1)).toBeNull();
    expect(titles(moveNode(tree(), [0], 1)!.nodes)).toEqual(["B", ["A", ["A1", "A2"]], "Web"]);
    const ind = indentNode(tree(), [1])!;
    expect(titles(ind.nodes)).toEqual([["A", ["A1", "A2", "B"]], "Web"]);
    expect(ind.path).toEqual([0, 2]);
    expect(indentNode(tree(), [0])).toBeNull();
    const out = outdentNode(ind.nodes, ind.path)!;
    expect(titles(out.nodes)).toEqual(titles(tree()));
    expect(out.path).toEqual([1]);
    expect(outdentNode(tree(), [1])).toBeNull();
  });
});
