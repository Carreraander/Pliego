export type Pt = [number, number];
export type Unit = "mm" | "cm" | "m" | "km" | "in" | "ft";
export const UNITS: Unit[] = ["mm", "cm", "m", "km", "in", "ft"];

/** Escala tal como la guarda y devuelve el servicio: unidades reales por punto del papel. */
export interface Scale { unit: Unit; units_per_point: number }
export interface Measurement { shape: "line" | "polyline" | "polygon"; length: number; area: number | null; unit: Unit; text: string }

/** Distancia en puntos del papel entre dos marcas. */
export const pageDistance = (a: Pt, b: Pt): number => Math.hypot(b[0] - a[0], b[1] - a[1]);

/** Lo que se está marcando: `calibrate` (dos puntos y una medida real), `distance` (línea abierta) o `area` (polígono). */
export type MeasureMode = "calibrate" | "distance" | "area";

/** Puntos que hacen falta para poder medir. */
export const minPoints = (mode: MeasureMode): number => (mode === "area" ? 3 : 2);

/** En calibración solo caben dos marcas; el resto de modos acepta las que haga falta. */
export const maxPoints = (mode: MeasureMode): number => (mode === "calibrate" ? 2 : Infinity);

/** Añade una marca respetando el máximo del modo (en calibración la tercera reinicia la pareja). */
export function addPoint(points: Pt[], p: Pt, mode: MeasureMode): Pt[] {
  return points.length >= maxPoints(mode) ? [p] : [...points, p];
}

/** Interpreta un número escrito con coma o punto decimal; `null` si no es positivo y finito. */
export function parsePositive(text: string): number | null {
  const v = Number(text.trim().replace(",", "."));
  return Number.isFinite(v) && v > 0 ? v : null;
}

/** Puntos de un `<polyline>`/`<polygon>` SVG en píxeles. */
export const svgPoints = (pts: Pt[], zoom: number): string => pts.map((p) => `${p[0] * zoom},${p[1] * zoom}`).join(" ");
