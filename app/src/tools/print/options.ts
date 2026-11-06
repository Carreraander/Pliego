/** Lógica pura del panel Imprimir: valores por defecto, texto de páginas y cuentas del pie. Sin DOM, para probarla. */
import type { Booklet, Orientation, PrintOptions } from "./api";

export function defaultOptions(): PrintOptions {
  return {
    printer: null, pages: "all", subset: "all", reverse: false, copies: 1, collate: true, orientation: "auto",
    autoRotate: true, scale: "fit", scalePercent: 100, duplex: "off", grayscale: false, comments: true, paper: null, output: null,
    pagesPerSheet: 1, nupOrder: "horizontal", borders: false, booklet: "off", bookletRtl: false,
  };
}

export type PageChoice = "all" | "current" | "range";

/** Expresión de páginas (desde 1) que entiende `pliego printer send` para lo elegido en el panel. */
export function pagesExpression(choice: PageChoice, current: number, range: string): string {
  if (choice === "all") return "all";
  if (choice === "current") return String(current);
  return range.trim();
}

/** Error del texto de un rango (`1-3,5`) contra `pageCount`, o `null` si vale. Devuelve una clave de texto. */
export function rangeError(text: string, pageCount: number): "empty" | "syntax" | "outside" | null {
  const parts = text.split(",").map((p) => p.trim()).filter(Boolean);
  if (parts.length === 0) return "empty";
  for (const part of parts) {
    const m = /^(\d+)(?:\s*-\s*(\d*))?$/.exec(part);
    if (!m) return "syntax";
    const a = Number(m[1]);
    const hasDash = part.includes("-");
    const b = m[2] ? Number(m[2]) : hasDash ? pageCount : a;
    if (a < 1 || b < 1 || a > pageCount || b > pageCount) return "outside";
  }
  return null;
}

/** Páginas (índices desde 0) que salen con una selección válida, en el orden en que se imprimen. */
export function selectedPages(expression: string, subset: PrintOptions["subset"], reverse: boolean, pageCount: number): number[] {
  const out: number[] = [];
  const push = (i: number) => { if (!out.includes(i)) out.push(i); };
  for (const raw of expression.split(",").map((p) => p.trim()).filter(Boolean)) {
    if (raw === "all") { for (let i = 0; i < pageCount; i++) push(i); continue; }
    const m = /^(\d+)(?:\s*-\s*(\d*))?$/.exec(raw);
    if (!m) continue;
    const a = Number(m[1]);
    const b = m[2] ? Number(m[2]) : raw.includes("-") ? pageCount : a;
    if (a <= b) for (let p = a; p <= b; p++) push(p - 1);
    else for (let p = a; p >= b; p--) push(p - 1);
  }
  let pages = out.filter((i) => i >= 0 && i < pageCount);
  if (subset === "odd") pages = pages.filter((i) => i % 2 === 0);
  if (subset === "even") pages = pages.filter((i) => i % 2 === 1);
  return reverse ? pages.reverse() : pages;
}

/** Cómo se reparte el documento en las hojas: una página por hoja, varias o folleto. */
export type SheetLayout = "1" | "2" | "4" | "6" | "9" | "16" | "booklet";

export const SHEET_LAYOUTS: SheetLayout[] = ["1", "2", "4", "6", "9", "16", "booklet"];

export function sheetLayout(o: Pick<PrintOptions, "pagesPerSheet" | "booklet">): SheetLayout {
  return o.booklet !== "off" ? "booklet" : (String(o.pagesPerSheet) as SheetLayout);
}

/** Opciones que cambian al elegir un reparto. El folleto empieza a doble cara y no combina con pares/impares. */
export function withSheetLayout(layout: SheetLayout, booklet: Booklet = "both"): Partial<PrintOptions> {
  if (layout === "booklet") return { pagesPerSheet: 1, booklet, subset: "all" };
  return { pagesPerSheet: Number(layout), booklet: "off" };
}

/** Con varias páginas por hoja (o folleto) la escala no se elige: cada página se ajusta a su celda. */
export function isMultiUp(o: Pick<PrintOptions, "pagesPerSheet" | "booklet">): boolean {
  return o.pagesPerSheet > 1 || o.booklet !== "off";
}

/** La hoja de la vista previa es apaisada: la elegida, o la de la primera página que sale (igual que al imprimir). */
export function landscapeSheet(orientation: Orientation, first: { width: number; height: number } | undefined): boolean {
  if (orientation === "portrait") return false;
  if (orientation === "landscape") return true;
  return !!first && first.width > first.height;
}

/** Tamaño del papel en milímetros con la orientación dada (los controladores lo dan siempre en vertical). */
export function paperSizeMm(widthMm: number, heightMm: number, landscape: boolean): [number, number] {
  const [short, long] = widthMm <= heightMm ? [widthMm, heightMm] : [heightMm, widthMm];
  return landscape ? [long, short] : [short, long];
}

/** Miniatura de la página: zoom con el que cabe en una sola tesela de 512 px. */
export function thumbZoom(width: number, height: number): number {
  return Math.min(1, 480 / Math.max(width, height, 1));
}

const KEY = "pliego.print.options";

/** Opciones de la última vez (impresora y ajustes, sin páginas ni copias). Solo una comodidad: si falla, vale lo de fábrica. */
export function loadRemembered(storage: Pick<Storage, "getItem"> | undefined = safeStorage()): Partial<PrintOptions> {
  try {
    const raw = storage?.getItem(KEY);
    if (!raw) return {};
    const v = JSON.parse(raw) as Partial<PrintOptions>;
    const keep: (keyof PrintOptions)[] = ["printer", "orientation", "autoRotate", "scale", "scalePercent", "duplex", "grayscale", "comments", "collate", "nupOrder", "borders"];
    return Object.fromEntries(Object.entries(v).filter(([k]) => keep.includes(k as keyof PrintOptions))) as Partial<PrintOptions>;
  } catch {
    return {};
  }
}

export function remember(o: PrintOptions, storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  try {
    storage?.setItem(KEY, JSON.stringify(o));
  } catch {
    /* sin almacenamiento: no pasa nada */
  }
}

function safeStorage(): Storage | undefined {
  try {
    return typeof localStorage === "undefined" ? undefined : localStorage;
  } catch {
    return undefined;
  }
}
