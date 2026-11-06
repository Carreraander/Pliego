import { describe, expect, it } from "vitest";
import {
  END, highlightRects, lineRange, moveWord, nearestWord, ordered, paragraphs, rangeOnPage, textOfRange, textOfSelection, wordAt,
  type Selection, type Word,
} from "./model";

const w = (text: string, x: number, y: number, block = 0, line = 0, width = text.length * 6): Word => ({ text, rect: [x, y, x + width, y + 10], block, line });
// «Hola mundo / segunda línea» en un bloque y «otro párrafo» en otro.
const page: Word[] = [
  w("Hola", 10, 10, 0, 0), w("mundo", 40, 10, 0, 0),
  w("segunda", 10, 24, 0, 1), w("línea", 56, 24, 0, 1),
  w("otro", 10, 60, 1, 0), w("párrafo", 40, 60, 1, 0),
];

describe("hit-test", () => {
  it("encuentra la palabra bajo el punto, con holgura", () => {
    expect(wordAt(page, 12, 12)).toBe(0);
    expect(wordAt(page, 45, 14)).toBe(1);
    expect(wordAt(page, 39.5, 12)).toBe(1); // en el hueco, dentro de la holgura de «mundo»
    expect(wordAt(page, 300, 300)).toBeNull();
  });
  it("la más cercana sirve en huecos y márgenes", () => {
    expect(nearestWord(page, 200, 12)).toBe(1); // a la derecha del final de la línea
    expect(nearestWord(page, 0, 26)).toBe(2); // margen izquierdo
    expect(nearestWord(page, 20, 40)).toBe(2); // entre la línea 2 y el otro bloque, más cerca de la 2
    expect(nearestWord([], 1, 1)).toBeNull();
  });
});

describe("selección", () => {
  const sel = (a: [number, number], f: [number, number]): Selection => ({ anchor: { page: a[0], word: a[1] }, focus: { page: f[0], word: f[1] } });
  it("ordena los extremos aunque se arrastre hacia atrás", () => {
    expect(ordered(sel([1, 3], [0, 5]))).toEqual([{ page: 0, word: 5 }, { page: 1, word: 3 }]);
  });
  it("tramo por página: primera, intermedias y última", () => {
    const s = sel([0, 4], [2, 1]);
    expect(rangeOnPage(s, 0, 6)).toEqual([4, 5]);
    expect(rangeOnPage(s, 1, 6)).toEqual([0, 5]);
    expect(rangeOnPage(s, 2, 6)).toEqual([0, 1]);
    expect(rangeOnPage(s, 3, 6)).toBeNull();
    expect(rangeOnPage(s, 1, 0)).toBeNull(); // página sin texto
  });
  it("«hasta el final» no necesita saber cuántas palabras hay", () => {
    expect(rangeOnPage(sel([0, 0], [0, END]), 0, 6)).toEqual([0, 5]);
  });
  it("una caja por línea, cubriendo los espacios", () => {
    expect(highlightRects(page, 0, 3)).toEqual([[10, 10, 70, 20], [10, 24, 86, 34]]);
    expect(highlightRects(page, 1, 4)).toEqual([[40, 10, 70, 20], [10, 24, 86, 34], [10, 60, 34, 70]]);
  });
  it("lineRange devuelve los extremos de la línea", () => {
    expect(lineRange(page, 3)).toEqual([2, 3]);
    expect(lineRange(page, 0)).toEqual([0, 1]);
  });
});

describe("texto copiado", () => {
  it("espacios, saltos de línea y líneas en blanco entre bloques", () => {
    expect(textOfRange(page, 0, 5)).toBe("Hola mundo\nsegunda línea\n\notro párrafo");
    expect(textOfRange(page, 1, 2)).toBe("mundo\nsegunda");
    expect(textOfRange(page, 4, 4)).toBe("otro");
  });
  it("une varias páginas con una línea en blanco", () => {
    const pages = [page, [w("fin", 5, 5)], []];
    const s: Selection = { anchor: { page: 0, word: 4 }, focus: { page: 2, word: END } };
    expect(textOfSelection(s, (p) => pages[p]!)).toBe("otro párrafo\n\nfin");
  });
  it("párrafos para lectores de pantalla", () => {
    expect(paragraphs(page)).toEqual(["Hola mundo segunda línea", "otro párrafo"]);
    expect(paragraphs([])).toEqual([]);
  });
});

describe("flechas", () => {
  it("izquierda y derecha recorren palabras y avisan en los bordes", () => {
    expect(moveWord(page, 1, "right")).toBe(2);
    expect(moveWord(page, 1, "left")).toBe(0);
    expect(moveWord(page, 0, "left")).toBeNull();
    expect(moveWord(page, 5, "right")).toBeNull();
  });
  it("arriba y abajo van a la palabra de la otra línea más cercana en horizontal", () => {
    expect(moveWord(page, 0, "down")).toBe(2); // «Hola» → «segunda»
    expect(moveWord(page, 1, "down")).toBe(3); // el centro de «mundo» (x≈55) queda más cerca de «línea» (56-86) que de «segunda» (10-52)
    expect(moveWord(page, 3, "up")).toBe(1);
    expect(moveWord(page, 0, "up")).toBeNull();
    expect(moveWord(page, 4, "down")).toBe(null);
  });
});
