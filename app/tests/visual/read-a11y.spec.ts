import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

/**
 * Accesibilidad del visor en Leer (Narrador y alto contraste) con el backend de Tauri simulado: roles y nombres, anuncios de
 * página y de selección, orden de Tab y foco visible, y selección legible con los colores del sistema. La prueba con el
 * Narrador de verdad está descrita en `docs/qa/narrador-visor.md` (acr12/pc).
 */
const DOC = "C:\\Docs\\informe.pdf";
const WORDS = [
  [
    { text: "Hola", rect: [72, 100, 102, 112], block: 0, line: 0 },
    { text: "mundo", rect: [108, 100, 140, 112], block: 0, line: 0 },
  ],
  [],
];
const LINKS = [
  [
    { rect: [72, 140, 200, 160], target: { type: "page", page: 1, y: null } },
    { rect: [72, 180, 200, 200], target: { type: "uri", uri: "https://pdfpliego.com/" } },
  ],
  [],
];

async function installBackend(page: Page) {
  await page.addInitScript(([doc, words, links]: [string, unknown, unknown]) => {
    const responses: Record<string, unknown> = {
      startup_files: [doc],
      open_document: 1,
      document_info: { page_count: 2, title: "Informe", author: null, is_pdf: true },
      page_sizes: [{ width: 595, height: 842 }, { width: 595, height: 842 }],
      outline: [],
      recents_list: [],
    };
    let nextId = 1;
    const w = window as unknown as Record<string, unknown>;
    const listeners: Record<string, number> = {};
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
      transformCallback: (cb: unknown) => {
        const id = nextId++;
        w[`_${id}`] = cb;
        return id;
      },
      unregisterCallback: () => undefined,
      convertFileSrc: (p: string) => p,
      invoke: async (cmd: string, args: { page?: number }) => {
        if (cmd === "plugin:event|listen") {
          const a = args as unknown as { event: string; handler: number };
          listeners[a.event] = a.handler;
          return nextId++;
        }
        if (cmd.startsWith("plugin:")) return null;
        if (cmd === "page_words") return (words as unknown[][])[args.page ?? 0] ?? [];
        if (cmd === "page_links") return (links as unknown[][])[args.page ?? 0] ?? [];
        return cmd in responses ? responses[cmd] : null;
      },
    };
  }, [DOC, WORDS, LINKS] as [string, unknown, unknown]);
}

async function open(page: Page) {
  await installBackend(page);
  await page.goto("/");
  await page.waitForSelector(".pl-window", { timeout: 45_000 });
  await page.waitForSelector('.page[data-page="0"] .pl-link');
}

test.describe("accesibilidad del visor en Leer (backend simulado)", () => {
  test("sin infracciones graves de axe y con nombres y roles esperados", async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "axe se pasa en una sola variante");
    await open(page);
    const graves = (await new AxeBuilder({ page }).include(".viewer").analyze()).violations
      .filter((v) => v.impact === "serious" || v.impact === "critical")
      .map((v) => ({ id: v.id, nodes: v.nodes.map((n) => n.html.slice(0, 140)) }));
    expect(graves).toEqual([]);
    await expect(page.getByRole("region", { name: "Documento: Informe" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Ir a la página 2" })).toBeAttached();
    await expect(page.locator('.page[data-page="0"] [role="group"] p')).toHaveText(["Hola mundo"]);
    // Una página sin texto se lo dice al lector de pantalla en lugar de quedar muda.
    await page.locator(".viewer").evaluate((el) => el.scrollTo(0, el.scrollHeight));
    await expect(page.locator('.page[data-page="1"] [role="group"] p')).toContainText("Sin texto que leer ni seleccionar");
  });

  test("Tab llega a los enlaces con foco visible e Intro los sigue", async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await open(page);
    await page.locator(".viewer").focus();
    await page.keyboard.press("Tab");
    const first = page.getByRole("link", { name: "Ir a la página 2" });
    await expect(first).toBeFocused();
    const outline = await first.evaluate((el) => getComputedStyle(el).outlineStyle + " " + getComputedStyle(el).outlineWidth);
    expect(outline).toMatch(/solid [1-9]/);
    await page.keyboard.press("Enter");
    await expect.poll(() => page.locator(".viewer").evaluate((el) => el.scrollTop)).toBeGreaterThan(842);
  });

  test("la región en vivo anuncia la página a la que se llega y lo que se selecciona con el teclado", async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await open(page);
    const live = page.getByTestId("viewer-live");
    await page.locator(".viewer").evaluate((el) => el.scrollTo(0, el.scrollHeight));
    await expect(live).toHaveText("Página 2 de 2");
    await page.locator(".viewer").evaluate((el) => el.scrollTo(0, 0));
    await expect(live).toHaveText("Página 1 de 2");
    await page.locator(".viewer").focus();
    await page.keyboard.press("Shift+ArrowRight");
    await expect(live).toHaveText("Seleccionado: mundo");
    await page.keyboard.press("Control+a");
    await expect(live).toHaveText("Todo el texto del documento seleccionado");
  });

  test("en alto contraste la selección y los enlaces siguen viéndose", async ({ page }, info) => {
    test.skip(info.project.name !== "alto-contraste", "solo con los colores forzados");
    await open(page);
    await page.locator(".viewer").focus();
    await page.keyboard.press("Control+a");
    const sel = page.locator(".pl-sel").first();
    await expect(sel).toBeAttached();
    const bg = await sel.evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(bg).not.toBe("rgba(0, 0, 0, 0)");
    const link = page.getByRole("link", { name: "Ir a la página 2" });
    expect(await link.evaluate((el) => getComputedStyle(el).outlineStyle)).toBe("solid");
  });
});
