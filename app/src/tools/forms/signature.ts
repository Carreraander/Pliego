/** «Mi firma»: trazos a mano normalizados (0-1 dentro de su caja) para guardarlos y colocarlos a cualquier tamaño. */
import type { Pt } from "../annotate/geometry";

export interface Signature { strokes: Pt[][]; /** ancho / alto de la caja original */ aspect: number }

const KEY = "pliego.signature";

/** Normaliza los trazos dibujados (en píxeles del lienzo) a su caja. `null` si no hay nada que firmar. */
export function normalize(strokes: Pt[][]): Signature | null {
  const pts = strokes.flat();
  if (pts.length < 2) return null;
  const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
  const x0 = Math.min(...xs), y0 = Math.min(...ys);
  const w = Math.max(...xs) - x0, h = Math.max(...ys) - y0;
  if (w < 2 && h < 2) return null;
  const size = Math.max(w, 1), height = Math.max(h, 1);
  return { strokes: strokes.map((s) => s.map((p): Pt => [(p[0] - x0) / size, (p[1] - y0) / height])), aspect: size / height };
}

/** Trazos en puntos de página para una firma de `width` puntos de ancho centrada en `at`. */
export function placeSignature(sig: Signature, at: Pt, width: number): Pt[][] {
  const height = width / sig.aspect;
  const left = at[0] - width / 2, top = at[1] - height / 2;
  return sig.strokes.map((s) => s.map((p): Pt => [left + p[0] * width, top + p[1] * height]));
}

export function loadSignature(): Signature | null {
  try {
    const raw = localStorage.getItem(KEY);
    const v = raw ? (JSON.parse(raw) as Signature) : null;
    return v && Array.isArray(v.strokes) && typeof v.aspect === "number" ? v : null;
  } catch { return null; }
}

export function saveSignature(sig: Signature | null): void {
  try { if (sig) localStorage.setItem(KEY, JSON.stringify(sig)); else localStorage.removeItem(KEY); } catch { /* sin almacenamiento */ }
}
