/**
 * Lector mínimo de código Rust para las pruebas de traducción: devuelve los literales de cadena del código que no es de
 * pruebas, sin comentarios, con sus líneas. No es un analizador completo: salta comentarios (también anidados),
 * literales de carácter (`'"'`), cadenas normales y crudas (`r#"…"#`), y corta el archivo en el primer `#[cfg(test)]` (o `#[cfg(all(test, …))]`).
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

export interface Literal { text: string; file: string; line: number; before: string }

export function rustFiles(dir: string, skip: (p: string) => boolean, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (skip(p)) continue;
    if (statSync(p).isDirectory()) rustFiles(p, skip, out);
    else if (name.endsWith(".rs") && name !== "tests.rs") out.push(p);
  }
  return out;
}

/** Literales de cadena de un archivo Rust (sin el módulo de pruebas del final). */
export function literalsOf(source: string, file: string): Literal[] {
  const test = /#\[cfg\((?:all\()?test\b/.exec(source);
  const src = test ? source.slice(0, test.index) : source;
  const out: Literal[] = [];
  let i = 0;
  let line = 1;
  const n = src.length;
  while (i < n) {
    const c = src[i]!;
    if (c === "\n") { line++; i++; continue; }
    if (c === "/" && src[i + 1] === "/") { while (i < n && src[i] !== "\n") i++; continue; }
    if (c === "/" && src[i + 1] === "*") {
      let depth = 1; i += 2;
      while (i < n && depth > 0) {
        if (src[i] === "\n") line++;
        if (src[i] === "/" && src[i + 1] === "*") { depth++; i += 2; } else if (src[i] === "*" && src[i + 1] === "/") { depth--; i += 2; } else i++;
      }
      continue;
    }
    // Literal de carácter ('"', '\'', 'a') frente a una vida útil ('a).
    if (c === "'") {
      const m = /^'(?:\\.[^']*|[^\\'])'/.exec(src.slice(i, i + 12));
      if (m) { i += m[0].length; continue; }
      i++; continue;
    }
    // Cadena cruda r"…", r#"…"#, br"…".
    const raw = c === "r" || (c === "b" && src[i + 1] === "r") ? /^b?r(#*)"/.exec(src.slice(i, i + 8)) : null;
    if (raw && !/[A-Za-z0-9_]/.test(src[i - 1] ?? " ")) {
      const hashes = raw[1]!;
      const end = `"${hashes}`;
      const start = i + raw[0].length;
      const close = src.indexOf(end, start);
      if (close < 0) break;
      const text = src.slice(start, close);
      out.push({ text, file, line, before: src.slice(Math.max(0, i - 300), i) });
      line += (text.match(/\n/g) ?? []).length;
      i = close + end.length;
      continue;
    }
    if (c === '"') {
      let j = i + 1;
      let text = "";
      const startLine = line;
      while (j < n && src[j] !== '"') {
        if (src[j] === "\\") {
          const nx = src[j + 1]!;
          if (nx === "\n" || (nx === "\r" && src[j + 2] === "\n")) {
            // Continuación de línea: se come el salto y el sangrado siguiente.
            j += nx === "\n" ? 2 : 3; line++;
            while (j < n && /[ \t\r\n]/.test(src[j]!)) { if (src[j] === "\n") line++; j++; }
            continue;
          }
          text += nx === "n" ? "\n" : nx === "t" ? "\t" : nx;
          j += 2; continue;
        }
        if (src[j] === "\n") line++;
        text += src[j]; j++;
      }
      out.push({ text, file, line: startLine, before: src.slice(Math.max(0, i - 300), i) });
      i = j + 1; continue;
    }
    i++;
  }
  return out;
}

export const relPath = (root: string, p: string): string => relative(root, p).replace(/\\/g, "/");
export const readText = (p: string): string => readFileSync(p, "utf8");
