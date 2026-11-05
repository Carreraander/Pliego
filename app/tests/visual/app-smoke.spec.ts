import { expect, test, type Page } from "@playwright/test";

/**
 * Humo de la app completa en el navegador con el backend de Tauri simulado: arranca la `App` de verdad (sin
 * `/nav-demo`), abre un documento y recorre OCR y Comparar comprobando los comandos que se envían. No sustituye a la
 * prueba en Windows, pero habría cazado que la interfaz saliera en inglés o que una herramienta siguiera «fantasma».
 */
const DOC = "C:\\Docs\\escaneo.pdf";

async function installBackend(page: Page) {
  await page.addInitScript((doc: string) => {
    const calls: { cmd: string; args: unknown }[] = [];
    const responses: Record<string, unknown> = {
      startup_files: [doc],
      open_document: 1,
      document_info: { page_count: 3, title: "Escaneo", author: null, is_pdf: true },
      page_sizes: [{ width: 595, height: 842 }, { width: 595, height: 842 }, { width: 595, height: 842 }],
      outline: [],
      recents_list: [],
      ocr_languages: { installed: ["eng", "osd", "spa"], downloadable: ["eng", "fra", "osd", "spa"] },
      pick_pdf_files: ["C:\\Docs\\version-2.pdf"],
      ocr_pdf: 11,
      compare_pdfs: 12,
      ocr_download_language: 13,
    };
    let nextId = 1;
    const w = window as unknown as Record<string, unknown>;
    w.__calls = calls;
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
      transformCallback: () => nextId++,
      unregisterCallback: () => undefined,
      convertFileSrc: (p: string) => p,
      invoke: async (cmd: string, args: unknown) => {
        calls.push({ cmd, args });
        if (cmd === "plugin:event|listen") return nextId++;
        if (cmd.startsWith("plugin:")) return null;
        return cmd in responses ? responses[cmd] : null;
      },
    };
  }, DOC);
}

const calls = (page: Page, cmd: string) =>
  page.evaluate((c) => ((window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls).filter((x) => x.cmd === c), cmd);

async function openTool(page: Page, query: string, option: RegExp) {
  await page.keyboard.press("Control+k");
  const box = page.getByRole("combobox", { name: "Paleta de órdenes" });
  await box.fill(query);
  await page.getByRole("option", { name: option }).first().click();
}

test.describe("humo de la app (backend simulado)", () => {
  // Windows en inglés, como en acr12: la app debe salir en español igualmente.
  test.use({ locale: "en-US" });

  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
  });

  test("arranca en español aunque el sistema esté en inglés y abre el documento", async ({ page }) => {
    await expect(page.locator(".pl-modebar__modes").getByRole("tab")).toHaveText(["Leer", "Comentar", "Rellenar y firmar", "Organizar", "Editar", "Proteger"]);
    expect((await calls(page, "open_document"))[0]?.args).toMatchObject({ path: DOC });
  });

  test("Reconocer texto: lista los idiomas instalados y lanza ocr_pdf con la petición esperada", async ({ page }) => {
    await openTool(page, "reconocer texto", /Reconocer texto/);
    await expect(page.getByRole("checkbox", { name: "Español (spa)" })).toBeChecked();
    await expect(page.getByRole("checkbox", { name: "Inglés (eng)" })).toBeChecked();
    await expect(page.getByRole("checkbox", { name: /osd/ })).toHaveCount(0);
    await page.getByRole("button", { name: "Reconocer y guardar copia" }).click();
    const sent = (await calls(page, "ocr_pdf"))[0];
    expect(sent?.args).toEqual({ request: { path: DOC, langs: "spa+eng", pages: "all", force: false, deskew: false, rotate: false, binarize: false, denoise: false, dpi: 300, suffix: "OCR" } });
  });

  test("Idiomas de OCR: ofrece descargar los que faltan", async ({ page }) => {
    await openTool(page, "idiomas de ocr", /Idiomas de OCR/);
    await expect(page.getByText("Francés (fra)")).toBeVisible();
    await page.getByRole("button", { name: "Descargar" }).click();
    expect((await calls(page, "ocr_download_language"))[0]?.args).toEqual({ code: "fra" });
  });

  test("Comparar: elige la otra versión y lanza compare_pdfs", async ({ page }) => {
    await openTool(page, "comparar", /^Comparar/);
    await page.getByRole("button", { name: "Elegir el otro PDF…" }).click();
    await page.getByRole("button", { name: "Comparar y guardar informe" }).click();
    expect((await calls(page, "compare_pdfs"))[0]?.args).toEqual({
      path: DOC, other: "C:\\Docs\\version-2.pdf", passwordA: null, passwordB: null, text: true, visual: true, label: "comparación",
    });
  });
});
