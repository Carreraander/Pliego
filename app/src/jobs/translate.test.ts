import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";
import { jobText, RULES } from "./translate";

describe("traducción de los textos de los trabajos", () => {
  it("en español no toca nada", () => {
    expect(jobText("Guardando", "es")).toBe("Guardando");
    expect(jobText("3 páginas numeradas", "es")).toBe("3 páginas numeradas");
  });

  it("traduce el progreso, con y sin parámetros", () => {
    expect(jobText("Guardando", "en")).toBe("Saving");
    expect(jobText("Listo", "en")).toBe("Done");
    expect(jobText("página 4", "en")).toBe("page 4");
    expect(jobText("Reconociendo página 12", "en")).toBe("Recognizing page 12");
    expect(jobText("Fila 2 de 9", "en")).toBe("Row 2 of 9");
    expect(jobText("Leyendo versión B, página 3", "en")).toBe("Reading versión B, page 3");
  });

  it("traduce los resúmenes y reordena los parámetros", () => {
    expect(jobText("12 páginas numeradas", "en")).toBe("12 pages numbered");
    expect(jobText("Imagen 7 borrada de la página 3", "en")).toBe("Image 7 deleted from page 3");
    expect(jobText("3 archivos combinados, 20 páginas", "en")).toBe("3 files merged, 20 pages");
    expect(jobText("Idioma «spa» instalado", "en")).toBe("Language “spa” installed");
    expect(jobText("5 procesados, 1 con error", "en")).toBe("5 processed, 1 failed");
    expect(jobText("Protección eliminada.", "en")).toBe("Protection removed.");
  });

  it("traduce también el resumen que lleva dentro otro resumen", () => {
    expect(jobText("3 página(s) creadas (cómic de 3 imágenes)", "en")).toBe("3 page(s) created (comic of 3 images)");
    expect(jobText("12 archivos creados (en C:\\Docs\\lote)", "en")).toBe("12 files created (in C:\\Docs\\lote)");
    expect(jobText("contrato.pdf: Listo", "en")).toBe("contrato.pdf: Done");
  });

  it("tolera saltos de línea y espacios de más", () => {
    expect(jobText("Convertido a PDF/A-2b   con la condición «sRGB».", "en")).toBe("Converted to PDF/A-2b with the condition “sRGB”.");
  });

  it("deja tal cual lo que no tiene regla (nombres de archivo, errores libres)", () => {
    expect(jobText("C:\\Docs\\contrato.pdf", "en")).toBe("C:\\Docs\\contrato.pdf");
    expect(jobText("", "en")).toBe("");
    expect(jobText("el PDF está dañado", "en")).toBe("el PDF está dañado");
  });

  it("ninguna regla inglesa conserva palabras españolas ni deja huecos sin rellenar", () => {
    for (const [es, en] of RULES) {
      expect(en, es).not.toMatch(/[áéíóúñ¿¡]/i);
      const slots = (es.match(/\{\w*\}/g) ?? []).length;
      const positional = [...en.matchAll(/\{(\d*)\}/g)].map((m) => m[1]);
      const used = positional.every((p) => p === "") ? positional.length : new Set(positional).size;
      expect(used, `${es} → ${en}`).toBe(slots);
    }
  });

  it("cada regla se puede alcanzar (ninguna plantilla anterior la tapa)", () => {
    for (const [es, en] of RULES) {
      const sample = es.replace(/\{\w*\}/g, "X");
      const got = jobText(sample, "en");
      expect(got, `${es} → ${en}`).toBe(en.replace(/\{\d*\}/g, "X"));
    }
  });
});

/**
 * Evita que el catálogo se quede atrás: busca en el código Rust las plantillas de progreso (`ctx.step`) y de resumen
 * (`Report::with_output`, `summary`, `warnings.push`) y exige que cada una tenga regla. Si añades un mensaje nuevo en
 * el motor y esta prueba falla, añade su traducción a `RULES` en `translate.ts`.
 */
describe("catálogo frente al código del motor", () => {
  const root = join(__dirname, "../../..");
  const skip = ["pliego-cli", join("pliego-pdf", "src", "native"), "tests", "target", "node_modules", ".git"];

  function rustFiles(dir: string, out: string[] = []): string[] {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name);
      if (skip.some((s) => relative(root, p).split(/[\\/]/).includes(s) || relative(root, p).includes(s))) continue;
      if (statSync(p).isDirectory()) rustFiles(p, out);
      else if (name.endsWith(".rs") && name !== "tests.rs") out.push(p);
    }
    return out;
  }

  const LIT = String.raw`"((?:[^"\\]|\\.)*)"`;
  const normalize = (t: string) =>
    t.replace(/\\\r?\n\s*/g, "").replace(/\\"/g, '"').replace(/\{[^}]*\}/g, "{}").replace(/\s+/g, " ").trim();

  function templates(): Map<string, string> {
    const found = new Map<string, string>();
    for (const file of [...rustFiles(join(root, "crates")), ...rustFiles(join(root, "app/service/src"))]) {
      let src = readFileSync(file, "utf8");
      const cut = src.indexOf("#[cfg(test)]");
      if (cut > 0) src = src.slice(0, cut);
      const patterns = [
        new RegExp(String.raw`\.step\(\s*[^;]*?(?:format!\(\s*${LIT}|${LIT})\s*[^;]*?\)\s*[?;]?`, "gs"),
        new RegExp(String.raw`(?:with_output\([^;]*?(?:format!\(\s*${LIT}|${LIT})|summary\s*[:=]\s*(?:format!\(\s*)?${LIT}|warnings\.push\((?:format!\(\s*)?${LIT})`, "gs"),
      ];
      for (const re of patterns) {
        for (const m of src.matchAll(re)) {
          for (const g of m.slice(1)) {
            if (g && /[A-Za-záéíóúñ]{3}/.test(g) && !g.startsWith("/")) found.set(normalize(g), relative(root, file));
          }
        }
      }
    }
    return found;
  }

  // Textos que se ven en el centro de tareas pero no son mensajes traducibles (nombres, rutas, etiquetas internas).
  const IGNORED = new Set<string>(["salida.pdf"]);

  it("todas las plantillas de progreso y resumen tienen traducción", () => {
    const known = new Set(RULES.map(([es]) => normalize(es)));
    const missing = [...templates()].filter(([t]) => !known.has(t) && !IGNORED.has(t)).map(([t, f]) => `${t}   (${f})`);
    expect(missing, "mensajes del motor sin traducir en jobs/translate.ts").toEqual([]);
  });
});
