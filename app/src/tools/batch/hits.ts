/** Resultado del índice (`pliego_index::Hit`) y su fragmento con las coincidencias marcadas. */
export interface IndexHit { path: string; page: number; score: number; snippet: string; highlights: [number, number][] }
export interface SnippetPart { text: string; mark: boolean }

/** Parte `snippet` en trozos normales y marcados. Los rangos vienen en bytes UTF-8, no en caracteres. */
export function snippetParts(snippet: string, highlights: [number, number][]): SnippetPart[] {
  const bytes = new TextEncoder().encode(snippet);
  const dec = new TextDecoder();
  const parts: SnippetPart[] = [];
  let at = 0;
  for (const [a, b] of [...highlights].sort((x, y) => x[0] - y[0])) {
    const start = Math.max(a, at), end = Math.min(b, bytes.length);
    if (start >= end) continue;
    if (start > at) parts.push({ text: dec.decode(bytes.slice(at, start)), mark: false });
    parts.push({ text: dec.decode(bytes.slice(start, end)), mark: true });
    at = end;
  }
  if (at < bytes.length) parts.push({ text: dec.decode(bytes.slice(at)), mark: false });
  return parts;
}
