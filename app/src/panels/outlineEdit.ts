/**
 * Edición del árbol de marcadores, pura (sin DOM ni Tauri) para probarla. Un marcador se señala con su ruta de índices
 * desde la raíz (`[2, 0]` = primer hijo del tercero). Cada operación devuelve un árbol nuevo y la ruta del marcador
 * afectado (para dejarlo seleccionado), o `null` si no se puede hacer.
 */

/** Lo mismo que `pliego_organize::OutlineNode`. */
export interface OutlineNode {
  title: string;
  page: number | null;
  children: OutlineNode[];
  /** Altura de destino en la página (coordenadas del PDF); se conserva tal cual. */
  top?: number | null;
  uri?: string | null;
}

export type NodePath = number[];
export interface EditResult { nodes: OutlineNode[]; path: NodePath }

const clone = (nodes: OutlineNode[]): OutlineNode[] => nodes.map((n) => ({ ...n, children: clone(n.children) }));

/** Lista que contiene al nodo de `path` (sus hermanos) dentro de `root`, o `null`. */
function siblings(root: OutlineNode[], path: NodePath): OutlineNode[] | null {
  let list = root;
  for (const i of path.slice(0, -1)) {
    const n = list[i];
    if (!n) return null;
    list = n.children;
  }
  return path.length && path[path.length - 1]! < list.length ? list : null;
}

export function nodeAt(root: OutlineNode[], path: NodePath): OutlineNode | null {
  const list = siblings(root, path);
  return list ? list[path[path.length - 1]!]! : null;
}

/** Añade un marcador a la página `page` detrás de `after` (al mismo nivel) o al final si no hay selección. */
export function addNode(root: OutlineNode[], after: NodePath | null, title: string, page: number): EditResult {
  const nodes = clone(root);
  const node: OutlineNode = { title, page, children: [] };
  const list = after ? siblings(nodes, after) : null;
  if (!after || !list) {
    nodes.push(node);
    return { nodes, path: [nodes.length - 1] };
  }
  const i = after[after.length - 1]! + 1;
  list.splice(i, 0, node);
  return { nodes, path: [...after.slice(0, -1), i] };
}

export function renameNode(root: OutlineNode[], path: NodePath, title: string): EditResult | null {
  const t = title.trim();
  const nodes = clone(root);
  const n = nodeAt(nodes, path);
  if (!n || !t) return null;
  n.title = t;
  return { nodes, path };
}

/** Cambia el destino a otra página (va a la página entera). */
export function retargetNode(root: OutlineNode[], path: NodePath, page: number): EditResult | null {
  const nodes = clone(root);
  const n = nodeAt(nodes, path);
  if (!n) return null;
  n.page = page;
  n.top = null;
  n.uri = null;
  return { nodes, path };
}

/** Borra el marcador y todo lo que cuelga de él. Deja seleccionado el siguiente, el anterior o el padre. */
export function deleteNode(root: OutlineNode[], path: NodePath): EditResult | null {
  const nodes = clone(root);
  const list = siblings(nodes, path);
  if (!list) return null;
  const i = path[path.length - 1]!;
  list.splice(i, 1);
  const parent = path.slice(0, -1);
  const next = list.length ? [...parent, Math.min(i, list.length - 1)] : parent;
  return { nodes, path: next };
}

/** Sube (-1) o baja (1) el marcador entre sus hermanos. */
export function moveNode(root: OutlineNode[], path: NodePath, dir: 1 | -1): EditResult | null {
  const nodes = clone(root);
  const list = siblings(nodes, path);
  if (!list) return null;
  const i = path[path.length - 1]!;
  const j = i + dir;
  if (j < 0 || j >= list.length) return null;
  [list[i], list[j]] = [list[j]!, list[i]!];
  return { nodes, path: [...path.slice(0, -1), j] };
}

/** Lo convierte en el último hijo del hermano anterior. */
export function indentNode(root: OutlineNode[], path: NodePath): EditResult | null {
  const nodes = clone(root);
  const list = siblings(nodes, path);
  const i = path[path.length - 1]!;
  if (!list || i === 0) return null;
  const [n] = list.splice(i, 1);
  const prev = list[i - 1]!;
  prev.children.push(n!);
  return { nodes, path: [...path.slice(0, -1), i - 1, prev.children.length - 1] };
}

/** Lo saca un nivel: pasa detrás de su padre. */
export function outdentNode(root: OutlineNode[], path: NodePath): EditResult | null {
  if (path.length < 2) return null;
  const nodes = clone(root);
  const list = siblings(nodes, path);
  const parentPath = path.slice(0, -1);
  const outer = siblings(nodes, parentPath);
  if (!list || !outer) return null;
  const [n] = list.splice(path[path.length - 1]!, 1);
  const at = parentPath[parentPath.length - 1]! + 1;
  outer.splice(at, 0, n!);
  return { nodes, path: [...parentPath.slice(0, -1), at] };
}

export const samePath = (a: NodePath | null, b: NodePath | null): boolean =>
  !!a && !!b && a.length === b.length && a.every((v, i) => v === b[i]);
