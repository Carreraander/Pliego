import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";
import en from "./en.json";
import es from "./es.json";

// Todas las claves de interfaz existen en español e inglés (una UI mezclada en Windows en inglés fue un fallo real).
const root = join(__dirname, "..");

function stringsFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) return stringsFiles(p);
    return /strings\.ts$/.test(n) ? [p] : [];
  });
}

function keys(block: string): string[] {
  return [...block.matchAll(/"([\w.-]+)":\s*(?:"|`)/g)].map((m) => m[1]!);
}

describe("paridad de idiomas", () => {
  it("es.json y en.json tienen las mismas claves", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(es).sort());
  });

  const files = stringsFiles(root);
  it("hay diccionarios que comprobar", () => expect(files.length).toBeGreaterThan(3));

  for (const file of files) {
    it(relative(root, file), () => {
      const src = readFileSync(file, "utf8");
      const esAt = src.search(/\bes:\s*\{/);
      const enAt = src.search(/\ben:\s*\{/);
      if (esAt < 0 || enAt < 0) return; // sin diccionario es/en en línea
      const [first, second] = esAt < enAt ? [src.slice(esAt, enAt), src.slice(enAt)] : [src.slice(enAt, esAt), src.slice(esAt)];
      const [esKeys, enKeys] = esAt < enAt ? [keys(first), keys(second)] : [keys(second), keys(first)];
      expect(esKeys.length).toBeGreaterThan(0);
      expect(enKeys.filter((k) => !esKeys.includes(k)), "claves solo en inglés").toEqual([]);
      expect(esKeys.filter((k) => !enKeys.includes(k)), "claves solo en español").toEqual([]);
    });
  }
});
