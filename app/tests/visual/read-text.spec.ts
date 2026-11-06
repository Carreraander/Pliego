import { expect, test, type Page } from "@playwright/test";

/**
 * Texto de las páginas en Leer (selección con ratón y teclado, copiar, menú contextual y texto para lectores de
 * pantalla) con el backend de Tauri simulado: las palabras con caja las da `page_words`. La extracción real, con
 * PDFium, se prueba en `pliego-engine` y en el worker.
 */
const DOC = "C:\\Docs\\informe.pdf";
const WORDS = [
  [
    { text: "Hola", rect: [72, 100, 102, 112], block: 0, line: 0 },
    { text: "mundo", rect: [108, 100, 140, 112], block: 0, line: 0 },
    { text: "segunda", rect: [72, 114, 120, 126], block: 0, line: 1 },
    { text: "línea", rect: [126, 114, 160, 126], block: 0, line: 1 },
    { text: "aparte", rect: [72, 200, 110, 212], block: 1, line: 0 },
  ],
  [{ text: "otra", rect: [72, 700, 100, 712], block: 0, line: 0 }, { text: "página", rect: [106, 700, 150, 712], block: 0, line: 0 }],
];

async function installBackend(page: Page) {
  await page.addInitScript(([doc, words]: [string, unknown]) => {
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
        if (cmd === "page_words") return (words as unknown[][])[args.page ?? 0] ?? [];
        return cmd in responses ? responses[cmd] : null;
      },
    };
  }, [DOC, WORDS] as [string, unknown]);
}

/** Posición en pantalla de un punto de la página `n` (en puntos de página). */
async function at(page: Page, n: number, x: number, y: number) {
  const r = (await page.locator(`.page[data-page="${n}"] .page-rot`).boundingBox())!;
  const scale = r.width / 595;
  return { x: r.x + x * scale, y: r.y + y * scale };
}

async function drag(page: Page, from: { x: number; y: number }, to: { x: number; y: number }) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 3 });
  await page.mouse.move(to.x, to.y, { steps: 3 });
  await page.mouse.up();
}

const clipboard = (page: Page) => page.evaluate(() => navigator.clipboard.readText());

// El portapapeles es uno solo para todo el navegador: las pruebas que lo leen no pueden correr a la vez.
test.describe.configure({ mode: "serial" });

test.describe("texto de las páginas en Leer (backend simulado)", () => {
  test.beforeEach(async ({ page, context }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.waitForSelector('.page[data-page="0"]');
  });

  test("arrastrar selecciona palabras por líneas y Ctrl+C las copia con sus saltos", async ({ page }) => {
    // De «mundo» (línea 1) a «segunda» (línea 2).
    await drag(page, await at(page, 0, 120, 106), await at(page, 0, 90, 120));
    await expect(page.locator('.page[data-page="0"] .pl-sel')).toHaveCount(2);
    await page.keyboard.press("Control+c");
    await expect.poll(() => clipboard(page)).toBe("mundo\nsegunda");
  });

  test("arrastrar hacia atrás da el mismo texto, y la selección cruza a la página siguiente", async ({ page }) => {
    await page.locator(".viewer").evaluate((el) => el.scrollTo(0, el.scrollHeight));
    await page.waitForSelector('.page[data-page="1"] .pl-sr-only');
    await drag(page, await at(page, 1, 130, 706), await at(page, 1, 80, 706));
    await page.keyboard.press("Control+c");
    await expect.poll(() => clipboard(page)).toBe("otra página");
  });

  test("doble clic elige una palabra, triple clic la línea, y un clic en blanco quita la selección", async ({ page }) => {
    const p = await at(page, 0, 150, 120);
    await page.mouse.dblclick(p.x, p.y);
    await expect(page.locator('.page[data-page="0"] .pl-sel')).toHaveCount(1);
    await page.keyboard.press("Control+c");
    await expect.poll(() => clipboard(page)).toBe("línea");
    await page.mouse.click(p.x, p.y, { clickCount: 3 });
    await page.keyboard.press("Control+c");
    await expect.poll(() => clipboard(page)).toBe("segunda línea");
    const blank = await at(page, 0, 400, 300);
    await page.mouse.click(blank.x, blank.y);
    await expect(page.locator(".pl-sel")).toHaveCount(0);
  });

  test("Mayús+flechas extienden la selección y Esc la quita; Ctrl+A selecciona todo el documento", async ({ page }) => {
    const hola = await at(page, 0, 85, 106);
    await page.mouse.dblclick(hola.x, hola.y);
    await page.keyboard.press("Shift+ArrowRight");
    await page.keyboard.press("Shift+ArrowDown");
    await page.keyboard.press("Control+c");
    await expect.poll(() => clipboard(page)).toBe("Hola mundo\nsegunda línea");
    await page.keyboard.press("Escape");
    await expect(page.locator(".pl-sel")).toHaveCount(0);
    await page.keyboard.press("Control+a");
    await page.keyboard.press("Control+c");
    await expect.poll(() => clipboard(page)).toBe("Hola mundo\nsegunda línea\n\naparte\n\notra página");
  });

  test("el menú contextual copia y selecciona todo en lugar del menú del navegador", async ({ page }) => {
    const p = await at(page, 0, 85, 106);
    await page.mouse.dblclick(p.x, p.y);
    await page.mouse.click(p.x, p.y, { button: "right" });
    await expect(page.getByRole("menuitem", { name: "Seleccionar todo" })).toBeVisible();
    await page.getByRole("menuitem", { name: /^Copiar/ }).click();
    await expect.poll(() => clipboard(page)).toBe("Hola");
  });

  test("las páginas ofrecen su texto a los lectores de pantalla, en orden de lectura", async ({ page }) => {
    const group = page.locator('.page[data-page="0"] [role="group"]');
    await expect(group).toHaveAttribute("aria-label", "Página 1 de 2");
    await expect(group.locator("p")).toHaveText(["Hola mundo segunda línea", "aparte"]);
  });
});
