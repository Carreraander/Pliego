/** Plancha tal como la devuelve `output_preview`. */
export interface PlatePreview { channel: "cyan" | "magenta" | "yellow" | "black"; coverage: number; png: string }
export interface InkInfo { name: string; kind: "process" | "spot"; pages: number[] }
export interface OutputPreview { plates: PlatePreview[]; max_total_ink: number; inks: InkInfo[] }

/** Límite habitual de tinta total en offset estucado (300 %). */
export const TAC_LIMIT = 3;

/** Porcentaje entero (0 a 100 en una plancha, hasta 400 en tinta total). */
export const percent = (fraction: number): number => Math.round(fraction * 100);

/** `true` si la tinta total pasa del límite: hay que avisar. */
export const overLimit = (maxTotalInk: number, limit = TAC_LIMIT): boolean => maxTotalInk > limit + 0.005;

/** Fuente de imagen de una plancha. */
export const plateSrc = (p: PlatePreview): string => `data:image/png;base64,${p.png}`;

/** Tintas directas (las de proceso se ven ya en las cuatro planchas). */
export const spotInks = (inks: InkInfo[]): InkInfo[] => inks.filter((i) => i.kind === "spot");
