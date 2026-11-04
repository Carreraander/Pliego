/** Utilidades puras de las herramientas de sellado (W27). */

export type Rgb = [number, number, number];

/** `#RRGGBB` (lo que da `<input type="color">`) a componentes de 0 a 1. */
export function hexToRgb(hex: string): Rgb {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  if (!m) return [0, 0, 0];
  return [parseInt(m[1]!, 16) / 255, parseInt(m[2]!, 16) / 255, parseInt(m[3]!, 16) / 255];
}

const PAGES = /^(all|todas|odd|impares|even|pares|(\d+|last)(-(\d+|last)?)?)(\s*,\s*(all|todas|odd|impares|even|pares|(\d+|last)(-(\d+|last)?)?))*$/i;

/** ¿Es un rango de páginas con la sintaxis de `PageRange` (`1-3,5,8-`, `odd`, `last`…)? El motor valida los límites. */
export function validPages(spec: string): boolean {
  const s = spec.trim();
  return s === "" || PAGES.test(s);
}

/** Texto de páginas que se envía: vacío significa todas. */
export const pagesOrAll = (spec: string): string => spec.trim() || "all";

/** Número entero en un rango, o `null`. */
export function intIn(text: string, min: number, max: number): number | null {
  const v = Number(text);
  return text.trim() !== "" && Number.isInteger(v) && v >= min && v <= max ? v : null;
}

/** Número decimal en un rango, o `null`. */
export function numIn(text: string, min: number, max: number): number | null {
  const v = Number(text.replace(",", "."));
  return text.trim() !== "" && Number.isFinite(v) && v >= min && v <= max ? v : null;
}

/** Primer número Bates tal como quedará: `<prefijo><número con ceros><sufijo>`. */
export function batesSample(prefix: string, suffix: string, digits: number, start: number): string {
  return `${prefix}${String(start).padStart(digits, "0")}${suffix}`;
}
