import { expect, test, type Page } from "@playwright/test";

/**
 * Enlaces del documento en Leer con el backend de Tauri simulado: `page_links` da los enlaces de cada página. Saltos
 * internos con historial (Alt+←/→), direcciones web con confirmación antes de abrir y rechazo de lo que no es web.
 * La lectura real de los enlaces (PDFium y el modelo de objetos) se prueba en `pliego-engine`.
 */
const DOC = "C:\\Docs\\informe.pdf";
const LINKS = [
  [
    { rect: [72, 100, 200, 120], target: { type: "page", page: 1, y: 300 } },
    { rect: [72, 140, 200, 160], target: { type: "uri", uri: "https://pdfpliego.com/ayuda?x=1" } },
    { rect: [72, 180, 200, 200], target: { type: "uri", uri: "file:///C:/Windows/System32/calc.exe" } },
  ],
  [],
];

async function installBackend(page: Page) {
  await page.addInitScript(([doc, links]: [string, unknown]) => {
    const calls: { cmd: string; args: unknown }[] = [];
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
    w.__calls = calls;
    const listeners: Record<string, number> = {};
    w.__emit = (event: string, payload: unknown) =>
      (w[`_${listeners[event]}`] as (e: unknown) => void)({ event, id: 0, payload });
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
        calls.push({ cmd, args });
        if (cmd === "plugin:event|listen") {
          const a = args as unknown as { event: string; handler: number };
          listeners[a.event] = a.handler;
          return nextId++;
        }
        if (cmd.startsWith("plugin:")) return null;
        if (cmd === "page_links") return (links as unknown[][])[args.page ?? 0] ?? [];
        if (cmd === "page_words") return [];
        return cmd in responses ? responses[cmd] : null;
      },
    };
  }, [DOC, LINKS] as [string, unknown]);
}

const calls = (page: Page, cmd: string) =>
  page.evaluate((c) => ((window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls).filter((x) => x.cmd === c), cmd);

const scrollTop = (page: Page) => page.locator(".viewer").evaluate((el) => el.scrollTop);

// El portapapeles es uno solo para todo el navegador.
test.describe.configure({ mode: "serial" });

test.describe("enlaces del documento en Leer (backend simulado)", () => {
  test.beforeEach(async ({ page, context }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.waitForSelector('.page[data-page="0"] .pl-link');
  });

  test("un enlace interno lleva a su página y Alt+← / Alt+→ recorren la historia de saltos", async ({ page }) => {
    expect(await scrollTop(page)).toBeLessThan(30);
    await page.getByRole("link", { name: "Ir a la página 2" }).click();
    await expect.poll(() => scrollTop(page)).toBeGreaterThan(842);
    const there = await scrollTop(page);
    await page.keyboard.press("Alt+ArrowLeft");
    await expect.poll(() => scrollTop(page)).toBeLessThan(120);
    await page.keyboard.press("Alt+ArrowRight");
    await expect.poll(() => scrollTop(page)).toBeGreaterThan(there - 20);
  });

  test("una dirección web pide confirmación y solo se abre al aceptar", async ({ page }) => {
    const link = page.getByRole("link", { name: /Abrir enlace: https:\/\/pdfpliego\.com/ });
    await link.click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByTestId("link-url")).toHaveText("https://pdfpliego.com/ayuda?x=1");
    await expect(dialog).toContainText("Sitio: pdfpliego.com");
    await dialog.getByRole("button", { name: "Cancelar" }).click();
    expect(await calls(page, "open_web_link")).toHaveLength(0);
    await link.click();
    await page.getByRole("dialog").getByRole("button", { name: "Abrir" }).click();
    const sent = await calls(page, "open_web_link");
    expect(sent).toHaveLength(1);
    expect(sent[0]!.args).toEqual({ url: "https://pdfpliego.com/ayuda?x=1" });
  });

  test("lo que no es web ni correo no llega a abrirse", async ({ page }) => {
    await page.getByRole("link", { name: /calc\.exe/ }).click();
    await expect(page.getByRole("dialog")).toBeHidden();
    expect(await calls(page, "open_web_link")).toHaveLength(0);
    await expect(page.getByText(/No se pudo abrir el enlace/)).toBeVisible();
  });

  test("el menú contextual de un enlace web copia su dirección", async ({ page }) => {
    await page.getByRole("link", { name: /Abrir enlace: https:\/\/pdfpliego\.com/ }).click({ button: "right" });
    await page.getByRole("menuitem", { name: "Copiar dirección del enlace" }).click();
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe("https://pdfpliego.com/ayuda?x=1");
  });
});
