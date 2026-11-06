/** Argumentos del comando `compare_pdfs` (puro, con pruebas). */

export interface CompareForm {
  path: string;
  other: string | null;
  /** Contraseña del segundo archivo, si está protegido (la del abierto la añade `withPassword`). */
  passwordB?: string | null;
  text: boolean;
  visual: boolean;
  label: string;
  /** Idioma del texto que el informe escribe en el PDF (el de la app). */
  lang?: "es" | "en";
}

/** Argumentos del comando, o `null` si falta el segundo documento, es el mismo que el abierto o no se compara nada. */
export function compareArgs(f: CompareForm): Record<string, unknown> | null {
  if (!f.path || !f.other || sameFile(f.path, f.other) || (!f.text && !f.visual)) return null;
  return { path: f.path, other: f.other, passwordA: null, passwordB: f.passwordB ?? null, text: f.text, visual: f.visual, label: f.label, lang: f.lang ?? null };
}

const norm = (p: string): string => p.replace(/\\/g, "/").toLowerCase();
export const sameFile = (a: string, b: string): boolean => norm(a) === norm(b);
