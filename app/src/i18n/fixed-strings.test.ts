import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

/**
 * Guardia de la interfaz: ningún texto visible puede estar escrito a fuego en un componente. Todo lo que ve la persona
 * pasa por una clave de `es.json`/`en.json` o por el diccionario de su área (`strings.ts`, `text.ts`). Esta prueba recorre
 * los `.tsx` de producción con el compilador de TypeScript y falla si encuentra:
 *  - texto suelto dentro de un elemento JSX (`<b>Guardar</b>`);
 *  - un atributo visible con una palabra literal (`title="Cerrar"`, `placeholder`, `aria-label`…);
 *  - una propiedad de objeto visible (`label: "Abrir"`, `title`, `hint`…) con una palabra literal.
 * Una clave (`"sign.title"`) o un valor sin letras (`"…"`, `"1"`) no cuentan. Lo que sea de verdad invariable (un nombre
 * propio de formato, una unidad) va en `ALLOWED`, con el motivo.
 */
const src = join(__dirname, "..");
// Pantallas de desarrollo (catálogo de componentes y demos): solo se cargan desde puntos de entrada de desarrollo.
const DEV_ONLY = new Set(["ui/Catalog.tsx", "shell/Demo.tsx", "navigation/Demo.tsx"]);

const ATTRS = new Set(["title", "aria-label", "aria-description", "placeholder", "alt", "label", "tooltip", "text", "hint", "caption", "description"]);
const PROPS = new Set(["title", "label", "text", "message", "hint", "description", "tooltip", "placeholder", "caption", "ariaLabel"]);

/** Textos invariables que sí pueden estar escritos a fuego. */
const ALLOWED = new Set<string>([
  "PDF/A", // nombre de formato
  "PDF", // nombre de formato
  "Pliego", // nombre del producto
  "AGPL-3.0-or-later", // licencia
  "http://…", // ejemplo de dirección en un campo de URL
]);

const files = (dir: string): string[] =>
  readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) return n === "node_modules" ? [] : files(p);
    return /\.tsx$/.test(n) && !/\.test\./.test(n) ? [p] : [];
  });

/** ¿Parece una clave de diccionario (`area.sub.nombre`) o un identificador técnico, no una frase? */
const isKey = (s: string): boolean => /^[a-z][\w-]*(\.[\w-]+)+$/.test(s);
/** Texto visible: tiene al menos una palabra de dos letras y no es una clave. */
const isWords = (s: string): boolean => /\p{L}{2}/u.test(s) && !isKey(s.trim()) && !ALLOWED.has(s.trim()) && !/^\W*$/u.test(s);

export function scan(rel: string, code: string): string[] {
  const out: string[] = [];
  {
    const sf = ts.createSourceFile(rel, code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    const at = (n: ts.Node, text: string) => out.push(`${rel}:${sf.getLineAndCharacterOfPosition(n.getStart()).line + 1}  ${JSON.stringify(text.trim())}`);
    const literal = (n: ts.Node | undefined): string | null =>
      n && (ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n)) ? n.text : null;
    const visit = (n: ts.Node) => {
      if (ts.isJsxText(n) && isWords(n.text)) at(n, n.text);
      if (ts.isJsxAttribute(n) && ATTRS.has(n.name.getText())) {
        const init = n.initializer;
        const v = init && ts.isStringLiteral(init) ? init.text : init && ts.isJsxExpression(init) ? literal(init.expression) : null;
        if (v !== null && isWords(v)) at(n, v);
      }
      if (ts.isPropertyAssignment(n) && PROPS.has(n.name.getText().replace(/["']/g, ""))) {
        const v = literal(n.initializer);
        if (v !== null && isWords(v)) at(n, v);
      }
      ts.forEachChild(n, visit);
    };
    visit(sf);
  }
  return out;
}

export function fixedStrings(): string[] {
  return files(src).flatMap((file) => {
    const rel = relative(src, file).replace(/\\/g, "/");
    return DEV_ONLY.has(rel) ? [] : scan(rel, readFileSync(file, "utf8"));
  });
}

describe("cadenas fijas en la interfaz", () => {
  it("el detector encuentra texto suelto, atributos y propiedades", () => {
    const code = `const a = { label: "Abrir archivo", key: "x.y" };
      export const C = () => <button title="Cerrar" aria-label={"cerrar"} placeholder="sign.hint">Guardar <b>{t("a.b")}</b></button>;`;
    expect(scan("x.tsx", code).map((l) => l.split("  ")[1])).toEqual(['"Abrir archivo"', '"Cerrar"', '"cerrar"', '"Guardar"']);
  });
  it("no marca claves, símbolos ni invariables", () => {
    expect(scan("x.tsx", `export const C = () => <i title="a.b" label="…">{"•"} PDF/A</i>;`)).toEqual([]);
  });
  it("ningún texto visible está escrito a fuego en un componente", () => {
    expect(fixedStrings(), "usa una clave de es.json/en.json (o del strings.ts del área), o añade el texto a ALLOWED si es invariable").toEqual([]);
  });
});
