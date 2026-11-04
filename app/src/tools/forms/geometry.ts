/** Del gesto del usuario (puntos de página) a la `FieldSpec` de `pliego_formdesign` (ver `app/service/src/formdesign.rs`). */

export type FieldKind = "text" | "check_box" | "radio_button" | "combo_box" | "list_box" | "push_button" | "signature";
export type DesignTool = "select" | FieldKind;
export type Pt = [number, number];
export type Box = [number, number, number, number];

/** Reflejo de `FieldSpec` con lo que la interfaz usa; `rect` está en espacio de página del visor. */
export interface FieldInfo {
  name: string; kind: FieldKind; page: number; rect: Box; required: boolean; read_only: boolean;
  value?: string | null; options: string[]; export_value?: string | null; multiline: boolean; tooltip?: string | null; max_len?: number | null;
}

/** Un arrastre menor que esto (puntos) es un clic y no crea campo. */
export const MIN_FIELD = 6;
/** Tamaño por defecto de un campo creado con un clic. */
export const DEFAULT_SIZE: Record<FieldKind, [number, number]> = {
  text: [140, 22], check_box: [14, 14], radio_button: [14, 14], combo_box: [140, 22], list_box: [140, 60], push_button: [90, 26], signature: [180, 50],
};

export function boxOf(a: Pt, b: Pt): Box {
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
}

/** Recuadro del campo nuevo: el arrastrado, o uno de tamaño por defecto si fue un clic. */
export function newBox(kind: FieldKind, a: Pt, b: Pt): Box {
  const box = boxOf(a, b);
  if (box[2] - box[0] >= MIN_FIELD && box[3] - box[1] >= MIN_FIELD) return box;
  const [w, h] = DEFAULT_SIZE[kind];
  return [a[0], a[1], a[0] + w, a[1] + h];
}

const PREFIX: Record<FieldKind, string> = {
  text: "Texto", check_box: "Casilla", radio_button: "Opcion", combo_box: "Desplegable", list_box: "Lista", push_button: "Boton", signature: "Firma",
};

/** Nombre libre `Prefijo1`, `Prefijo2`… sin puntos ni espacios (los nombres de campo no admiten puntos). */
export function freeName(kind: FieldKind, taken: Iterable<string>): string {
  const used = new Set(taken);
  let n = 1;
  while (used.has(`${PREFIX[kind]}${n}`)) n++;
  return `${PREFIX[kind]}${n}`;
}

/** Valida un nombre de campo; devuelve la clave de error de texto o `null`. */
export function nameError(name: string, taken: Iterable<string>, current?: string): string | null {
  const n = name.trim();
  if (!n) return "forms.err.nameEmpty";
  if (n.includes(".")) return "forms.err.nameDot";
  if (n !== current && [...taken].includes(n)) return "forms.err.nameTaken";
  return null;
}

/** Opciones de lista: una por línea, sin vacías ni repetidas. */
export function parseOptions(text: string): string[] {
  const out: string[] = [];
  for (const l of text.split(/\r?\n/)) {
    const v = l.trim();
    if (v && !out.includes(v)) out.push(v);
  }
  return out;
}

export function newSpec(kind: FieldKind, page: number, rect: Box, name: string, extra: Partial<FieldInfo> = {}): Record<string, unknown> {
  const spec: Record<string, unknown> = { name, kind, page, rect, ...extra };
  if (kind === "radio_button" && !spec.export_value) spec.export_value = name;
  if ((kind === "combo_box" || kind === "list_box") && !(spec.options as string[] | undefined)?.length) spec.options = ["Opción 1", "Opción 2"];
  if (kind === "push_button" && !spec.caption) spec.caption = name;
  return spec;
}

/** Cambios de `FieldPatch` entre el campo actual y los valores del panel; solo lo que cambió. */
export function patchOf(cur: FieldInfo, next: { name: string; required: boolean; read_only: boolean; tooltip: string; options: string[]; value: string }): Record<string, unknown> {
  const p: Record<string, unknown> = {};
  if (next.name !== cur.name) p.name = next.name;
  if (next.required !== cur.required) p.required = next.required;
  if (next.read_only !== cur.read_only) p.read_only = next.read_only;
  if (next.tooltip !== (cur.tooltip ?? "")) p.tooltip = next.tooltip;
  if (next.value !== (cur.value ?? "")) p.value = next.value;
  if ((cur.kind === "combo_box" || cur.kind === "list_box") && next.options.join("\n") !== cur.options.join("\n")) p.options = next.options;
  return p;
}
