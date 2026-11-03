/** Normaliza para comparar: minúsculas, sin tildes ni signos diacríticos. */
export function normalize(text: string): string {
  return text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

/**
 * Puntuación de coincidencia aproximada (mayor = mejor; `null` = no coincide). Sin tildes ni mayúsculas.
 * Todas las palabras de la consulta deben aparecer: como prefijo de palabra (mejor), como subcadena, o como
 * subsecuencia de letras (peor). El inicio del texto puntúa más.
 */
export function fuzzyScore(query: string, text: string, loose = true): number | null {
  const q = normalize(query).trim();
  if (!q) return 0;
  const target = normalize(text);
  let total = 0;
  for (const word of q.split(/\s+/)) {
    const s = wordScore(word, target, loose);
    if (s === null) return null;
    total += s;
  }
  return total;
}

function wordScore(word: string, target: string, loose: boolean): number | null {
  const at = target.indexOf(word);
  if (at >= 0) {
    const wordStart = at === 0 || /[\s\-_/.]/.test(target[at - 1] ?? "");
    return (wordStart ? 100 : 60) - Math.min(at, 30) / 2 - (target.length - word.length) / 20;
  }
  if (!loose) return null;
  // Subsecuencia: las letras en orden, con huecos.
  let pos = -1;
  let gaps = 0;
  for (const ch of word) {
    const next = target.indexOf(ch, pos + 1);
    if (next < 0) return null;
    gaps += pos < 0 ? 0 : next - pos - 1;
    pos = next;
  }
  return Math.max(1, 30 - gaps);
}

/** El mejor valor sobre varios textos. Los `exact` (descripciones) solo cuentan si contienen las palabras, sin subsecuencias. */
export function bestScore(query: string, texts: string[], exact: string[] = []): number | null {
  let best: number | null = null;
  for (const [t, loose] of [...texts.map((x) => [x, true] as const), ...exact.map((x) => [x, false] as const)]) {
    const s = fuzzyScore(query, t, loose);
    if (s !== null && (best === null || s > best)) best = s;
  }
  return best;
}
