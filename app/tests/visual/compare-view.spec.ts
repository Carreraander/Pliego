import { expect, test, type Page } from "@playwright/test";
import { mkdirSync } from "node:fs";

/**
 * Comparar, vista lado a lado, con el backend de Tauri simulado y las teselas pintadas por la propia prueba (un papel con
 * líneas de texto): comprueba que se abre, que las dos columnas comparten desplazamiento, que los cambios salen
 * resaltados en los dos documentos y que F3 salta de uno a otro. Las capturas van a `test-results/compare-view/`.
 */
const DOC = "C:\\Docs\\contrato-v1.pdf";
const OTHER = "C:\\Docs\\contrato-v2.pdf";
const SIZE = { width: 595, height: 842 };
const r = (y0: number, x1 = 300) => ({ x0: 72, y0, x1, y1: y0 + 12 });
const COMPARISON = {
  path_a: DOC, path_b: OTHER,
  summary: { pages_a: 2, pages_b: 3, inserted_pages: 1, deleted_pages: 0, changed_pages: 3, added_words: 7, removed_words: 3, changed_words: 2, format_changes: 1, visual_regions: 1 },
  pages: [
    { status: "matched", page_a: 0, page_b: 0, similarity: 0.9, visual_regions: [],
      text_changes: [
        { kind: "removed", page_a: 0, page_b: 0, text_a: "treinta días", text_b: "", rects_a: [r(100)], rects_b: [], format_only: false, note: null },
        { kind: "added", page_a: 0, page_b: 0, text_a: "", text_b: "sesenta días naturales", rects_a: [], rects_b: [r(100, 360)], format_only: false, note: null },
        { kind: "changed", page_a: 0, page_b: 0, text_a: "Madrid", text_b: "Bilbao", rects_a: [r(250, 200)], rects_b: [r(250, 200)], format_only: false, note: null },
        { kind: "changed", page_a: 0, page_b: 0, text_a: "Cláusula", text_b: "Cláusula", rects_a: [r(400, 180)], rects_b: [r(400, 180)], format_only: true, note: "Fuente: Arial 11 → Georgia 11" },
      ] },
    { status: "matched", page_a: 1, page_b: 1, similarity: 1, text_changes: [], visual_regions: [{ rect: { x0: 400, y0: 600, x1: 520, y1: 680 }, changed_pixels: 900 }] },
    { status: "inserted", page_a: null, page_b: 2, similarity: 0, text_changes: [], visual_regions: [] },
  ],
};

async function installBackend(page: Page) {
  await page.addInitScript(([doc, other, comparison, size]: [string, string, unknown, { width: number; height: number }]) => {
    const calls: { cmd: string; args: unknown }[] = [];
    let nextId = 1;
    let nextDoc = 1;
    const w = window as unknown as Record<string, unknown>;
    w.__calls = calls;
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
      transformCallback: () => nextId++,
      unregisterCallback: () => undefined,
      convertFileSrc: (p: string) => p,
      invoke: async (cmd: string, args: { docId?: number }) => {
        calls.push({ cmd, args });
        switch (cmd) {
          case "startup_files": return [doc];
          case "open_document": return nextDoc++;
          case "document_info": return { page_count: 2, title: "Contrato", author: null, is_pdf: true };
          case "page_sizes": return Array.from({ length: args.docId === 1 ? 2 : 3 }, () => size);
          case "pick_pdf_files": return [other];
          case "password_status": return "open";
          case "compare_view": return comparison;
          case "plugin:event|listen": return nextId++;
          case "outline": case "recents_list": return [];
          default: return cmd.startsWith("plugin:") ? null : null;
        }
      },
    };
  }, [DOC, OTHER, COMPARISON, SIZE] as [string, string, unknown, { width: number; height: number }]);

  // Teselas: un papel con líneas de texto (cada 24 pt desde y = 100) para que los resaltados caigan sobre «texto».
  await page.route("http://pliego.localhost/tile/**", (route) => {
    const [, , doc, pg, zoom, col, row] = new URL(route.request().url()).pathname.split("/");
    const z = Number(zoom);
    const lines = Array.from({ length: 28 }, (_, i) => `<rect x="72" y="${100 + i * 24 + 2}" width="${300 + ((i * 53) % 150)}" height="8" fill="#9aa19c"/>`).join("");
    const mark = `<text x="72" y="60" font-family="sans-serif" font-size="14" fill="#444">Documento ${doc === "1" ? "A" : "B"} · página ${Number(pg) + 1}</text>`;
    route.fulfill({
      contentType: "image/svg+xml",
      body: `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512"><rect width="512" height="512" fill="#fff"/><g transform="scale(${z}) translate(${-Number(col) * 512 / z} ${-Number(row) * 512 / z})">${mark}${lines}</g></svg>`,
    });
  });
}

test.describe("Comparar lado a lado (backend simulado)", () => {
  test.use({ userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120 Safari/537.36", locale: "es-ES" });

  test("abre las dos columnas, resalta los cambios y salta con F3", async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    mkdirSync("test-results/compare-view", { recursive: true });
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });

    await page.keyboard.press("Control+k");
    await page.getByRole("combobox", { name: "Paleta de órdenes" }).fill("comparar");
    await page.getByRole("option", { name: /^Comparar/ }).first().click();
    await page.getByRole("button", { name: "Elegir el otro PDF…" }).click();
    await page.getByRole("button", { name: "Ver lado a lado" }).click();

    const view = page.getByTestId("compare-view");
    await expect(view).toBeVisible();
    const sent = await page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: unknown }[] }).__calls.find((c) => c.cmd === "compare_view")?.args);
    expect(sent).toMatchObject({ path: DOC, other: OTHER, text: true, visual: true });

    // Dos columnas con el nombre de cada archivo.
    await expect(view.getByText("contrato-v1.pdf")).toBeVisible();
    await expect(view.getByText("contrato-v2.pdf")).toBeVisible();
    // El primer cambio queda a la vista al abrir: quitado a la izquierda, añadido a la derecha.
    await expect(view.locator('[data-side="a"] .cmp-mark[data-kind="removed"]').first()).toBeVisible();
    await expect(view.locator('[data-side="b"] .cmp-mark[data-kind="added"]').first()).toBeVisible();
    await expect(page.getByText("1 de 5")).toBeVisible();
    await page.screenshot({ path: "test-results/compare-view/1-primer-cambio.png" });

    // Misma fila, misma altura: el desplazamiento es uno solo.
    const tops = await view.locator('.cmp-row[data-row="0"] .cmp-page').evaluateAll((els) => els.map((e) => Math.round(e.getBoundingClientRect().top)));
    expect(tops[0]).toBe(tops[1]);

    await page.keyboard.press("F3");
    await expect(page.getByText("2 de 5")).toBeVisible();
    await page.keyboard.press("F3");
    await expect(page.getByText("3 de 5")).toBeVisible();
    await page.keyboard.press("F3");
    await expect(page.getByText("4 de 5")).toBeVisible();
    // Página 2: región de aspecto marcada en las dos.
    await expect(view.locator('.cmp-mark[data-kind="visual"]')).toHaveCount(2);
    await page.screenshot({ path: "test-results/compare-view/2-aspecto.png" });
    await page.keyboard.press("F3");
    await expect(page.getByText("5 de 5")).toBeVisible();
    // Página insertada: hueco en el original.
    await expect(view.getByTestId("compare-gap")).toBeVisible();
    await page.screenshot({ path: "test-results/compare-view/3-pagina-insertada.png" });
    // Tras el último, F3 da la vuelta al primero; antes del primero, Mayús+F3 va al último.
    await page.keyboard.press("F3");
    await expect(page.getByText("1 de 5")).toBeVisible();
    await page.keyboard.press("Shift+F3");
    await expect(page.getByText("5 de 5")).toBeVisible();
    await page.keyboard.press("Shift+F3");
    await expect(page.getByText("4 de 5")).toBeVisible();

    // Quitar el resaltado del texto deja solo lo del aspecto.
    await page.keyboard.press("Shift+F3");
    await page.keyboard.press("Shift+F3");
    await page.getByRole("checkbox", { name: "Resaltar el texto" }).uncheck();
    await expect(view.locator('.cmp-mark[data-kind="removed"]')).toHaveCount(0);

    // Cerrar suelta el segundo documento.
    await view.getByRole("button", { name: "Cerrar la comparación" }).click();
    await expect(view).toHaveCount(0);
    const closed = await page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: { docId?: number } }[] }).__calls.filter((c) => c.cmd === "close_document").map((c) => c.args.docId));
    expect(closed).toContain(2);
  });
});
