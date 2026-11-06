import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { jobText } from "../../jobs/translate";
import { NOT_TRANSLATED } from "./not-translated";
import { literalsOf, readText, relPath, rustFiles } from "./rust-literals";
import { looksSpanish, stillSpanish } from "./spanish";

/**
 * Guardia de traducción del motor. Los crates de Rust y `app/service` escriben sus mensajes (errores, informes, avisos)
 * en español y la interfaz los traduce con las reglas de `jobs/translate.ts` y `i18n/engine/`. Esta prueba lee todos los
 * literales de cadena del código de producción y falla si alguno que parece español no se traduce al inglés: añade la
 * regla en `i18n/engine/` (o, si no es un texto que vea nadie, explícalo en `not-translated.ts`).
 */
const root = join(__dirname, "../../../..");
// Fuera del barrido: la CLI y el instalador tienen su propio mecanismo; el resto son herramientas de desarrollo.
const SKIP = ["target", "node_modules", ".git", "tests", "native", "examples", "benches", "pliego-cli", "pliego-setup", "pliego-release", "pliego-qa", "pliego-install"];
const skip = (p: string) => relPath(root, p).split("/").some((s) => SKIP.includes(s));
// Llamadas cuyo texto no llega a nadie: registros, afirmaciones, `expect`, pánicos.
const SILENT = /(?:log::\w+|expect|expect_err|panic!|unreachable!|assert\w*!|debug_assert\w*!|todo!|unimplemented!|eprintln!|dbg!)\s*\(\s*(?:"[^"]*"\s*,\s*)?(?:&?format!\(\s*)?$/;

// Textos que el propio código escribe en el idioma de la app (un parámetro `lang`): `lang.pick("es", "en")` o la rama `::Es =>`
// de un `match`. Se reconocen mirando atrás, dentro de la misma sentencia, la llamada o la rama que los contiene.
const LOCALIZED = /(?:\.pick\(|::Es\s*=>)[^;{}]*$/;

export const normalize = (t: string): string => t.replace(/\{[^}]*\}/g, "{}").replace(/\s+/g, " ").trim();

export function productionLiterals(): Map<string, string[]> {
  const found = new Map<string, string[]>();
  const files = [...rustFiles(join(root, "crates"), skip), ...rustFiles(join(root, "app/service/src"), skip), ...rustFiles(join(root, "app/src-tauri/src"), skip)];
  for (const file of files) {
    for (const lit of literalsOf(readText(file), file)) {
      if (SILENT.test(lit.before) || LOCALIZED.test(lit.before)) continue;
      const key = normalize(lit.text);
      if (key.length < 4 || !looksSpanish(key)) continue;
      const at = `${relPath(root, file)}:${lit.line}`;
      found.set(key, [...(found.get(key) ?? []), at]);
    }
  }
  return found;
}

describe("guardia: todo texto del motor tiene traducción", () => {
  it("cada literal en español de los crates y del servicio se traduce al inglés", () => {
    const missing: string[] = [];
    for (const [template, where] of productionLiterals()) {
      if (NOT_TRANSLATED[template]) continue;
      const sample = template.replace(/\{\}/g, "ZZ");
      if (stillSpanish(jobText(sample, "en").replace(/ZZ/g, ""))) missing.push(`${where[0]}\t${template}`);
    }
    if (process.env.PLIEGO_I18N_DUMP) writeFileSync(process.env.PLIEGO_I18N_DUMP, missing.join("\n"));
    expect(missing, "textos del motor sin traducir (añade la regla en app/src/i18n/engine/ o justifícalo en not-translated.ts)").toEqual([]);
  });

  it("no sobra ninguna excepción en not-translated.ts", () => {
    const present = productionLiterals();
    const stale = Object.keys(NOT_TRANSLATED).filter((k) => !present.has(k));
    expect(stale, "excepciones que ya no existen en el código").toEqual([]);
  });
});
