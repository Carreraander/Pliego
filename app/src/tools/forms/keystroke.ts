/** Una edición de un campo de texto vista como lo ve el script de tecleo de Acrobat: `change` reemplaza `prev[start..end]`. */
export interface TextEdit { start: number; end: number; change: string }

/** Resultado de `forms_keystroke` (reflejo de `pliego_forms::KeystrokeResult`). */
export interface KeystrokeResult { accepted: boolean; change: string; messages: string[] }

/** Deduce qué se tecleó comparando el texto anterior con el nuevo (prefijo y sufijo comunes). `null` si no cambió. */
export function describeEdit(prev: string, next: string): TextEdit | null {
  if (prev === next) return null;
  const a = [...prev], b = [...next];
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let tail = 0;
  while (tail < a.length - start && tail < b.length - start && a[a.length - 1 - tail] === b[b.length - 1 - tail]) tail++;
  return { start, end: a.length - tail, change: b.slice(start, b.length - tail).join("") };
}

/** Texto que queda tras aplicar el veredicto del script a la edición: se revierte si lo rechazó y se usa su cambio si lo modificó. */
export function applyKeystroke(prev: string, edit: TextEdit, r: KeystrokeResult): { value: string; caret: number } {
  if (!r.accepted) return { value: prev, caret: edit.end };
  const a = [...prev];
  const value = [...a.slice(0, edit.start), ...r.change, ...a.slice(edit.end)].join("");
  return { value, caret: edit.start + [...r.change].length };
}
