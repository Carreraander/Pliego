import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// Ningún componente del armazón usa colores literales: todo sale de `tokens.css` (docs/design/sistema-visual.md).
const root = join(__dirname, "..");
const dirs = ["ui", "shell", "styles"].map((d) => join(root, d));
const hex = /(?<![&\w])#[0-9a-fA-F]{3,8}\b/;
const fn = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/i;

function files(dir: string): string[] {
  try {
    return readdirSync(dir).flatMap((n) => {
      const p = join(dir, n);
      return statSync(p).isDirectory() ? files(p) : /\.(css|tsx?)$/.test(n) && !n.endsWith(".test.ts") ? [p] : [];
    });
  } catch { return []; }
}

describe("colores literales", () => {
  const targets = dirs.flatMap(files).filter((f) => !f.endsWith(join("styles", "tokens.css")) && !f.endsWith("provisional.css")); // provisional.css (W01) desaparece con el armazón nuevo
  it("hay archivos que comprobar", () => expect(targets.length).toBeGreaterThan(5));
  for (const f of targets) {
    it(relative(root, f), () => {
      const bad = readFileSync(f, "utf8").split("\n").map((l, i) => [i + 1, l] as const)
        .filter(([, l]) => !l.trim().startsWith("//") && !l.trim().startsWith("*") && (hex.test(l) || fn.test(l)));
      expect(bad, `colores literales en ${f}`).toEqual([]);
    });
  }
});

describe("tokens.css", () => {
  it("es copia exacta de docs/design/tokens.css cuando existe", () => {
    const src = join(__dirname, "../../../docs/design/tokens.css");
    let design: string;
    try { design = readFileSync(src, "utf8"); } catch { return; }
    expect(readFileSync(join(root, "styles/tokens.css"), "utf8")).toBe(design);
  });
});
