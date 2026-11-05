import { expect, test, type Page } from "@playwright/test";

/**
 * Rellenar y firmar con scripts (W42, PR 5) con el backend de Tauri simulado: texto formateado sin enfocar y valor al
 * editar, filtro de tecleo (solo cifras), confirmación con `forms_fill` y botón con acción.
 */
const DOC = "C:\\Docs\\factura.pdf";

async function installBackend(page: Page) {
  await page.route("http://pliego.localhost/tile/**", (r) => r.fulfill({ status: 200, contentType: "image/png", body: Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==", "base64") }));
  await page.addInitScript((doc: string) => {
    const calls: { cmd: string; args: Record<string, unknown> }[] = [];
    const fields = [
      { name: "importe", kind: "text", value: "1234.5", display: "1.234,50 €", scripts: ["K", "F"], options: [], page: 0, rect: [40, 40, 240, 64], read_only: false, multiline: false, password: false, signed: false },
      { name: "nombre", kind: "text", value: "", options: [], page: 0, rect: [40, 80, 240, 104], read_only: false, multiline: false, password: false, signed: false },
      { name: "enviar", kind: "push_button", value: null, scripts: ["U"], options: [], page: 0, rect: [40, 120, 140, 150], read_only: false, multiline: false, password: false, signed: false },
    ];
    let nextId = 1;
    const w = window as unknown as Record<string, unknown>;
    w.__calls = calls;
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
      transformCallback: () => nextId++,
      unregisterCallback: () => undefined,
      convertFileSrc: (p: string) => p,
      invoke: async (cmd: string, args: Record<string, unknown>) => {
        calls.push({ cmd, args });
        switch (cmd) {
          case "startup_files": return [doc];
          case "open_document": return 1;
          case "document_info": return { page_count: 1, title: "Factura", author: null, is_pdf: true };
          case "page_sizes": return [{ width: 595, height: 842 }];
          case "outline": case "recents_list": return [];
          case "forms_fields": return fields;
          // Filtro de tecleo de un campo numérico: solo cifras y separador decimal.
          case "forms_keystroke": {
            const change = args.change as string;
            return { accepted: /^[0-9.,]*$/.test(change), change, messages: /^[0-9.,]*$/.test(change) ? [] : ["Solo se admiten cifras"] };
          }
          case "forms_fill": return { undo_depth: 1, messages: [] };
          case "forms_run_action": return { undo_depth: 1, messages: ["Factura enviada (simulado)"] };
          case "plugin:event|listen": return nextId++;
          default: return null;
        }
      },
    };
  }, DOC);
}

const calls = (page: Page, cmd: string) =>
  page.evaluate((c) => ((window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls).filter((x) => x.cmd === c), cmd);

test.describe("Rellenar con scripts (backend simulado)", () => {
  test.use({ locale: "es-ES", userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36" });

  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.getByRole("tab", { name: "Rellenar y firmar" }).click();
    await expect(page.getByTestId("fields-layer").first()).toBeVisible();
  });

  test("texto formateado sin enfocar, valor al editar y filtro de tecleo", async ({ page }, info) => {
    const importe = page.getByLabel("importe");
    await expect(importe).toHaveValue("1.234,50 €");
    await page.screenshot({ path: info.outputPath("rellenar-sin-enfocar.png") });
    await importe.focus();
    await expect(importe).toHaveValue("1234.5");
    // Letras: el script las rechaza y el texto vuelve a lo anterior.
    await importe.press("End");
    await page.keyboard.type("x");
    await expect.poll(async () => (await calls(page, "forms_keystroke")).length).toBeGreaterThan(0);
    await expect(importe).toHaveValue("1234.5");
    // Cifras: se aceptan.
    await page.keyboard.type("7");
    await expect(importe).toHaveValue("1234.57");
    await page.screenshot({ path: info.outputPath("rellenar-editando.png") });
    await importe.blur();
    const fill = await calls(page, "forms_fill");
    expect(fill[0]?.args).toMatchObject({ docId: 1, path: DOC, values: { importe: "1234.57" } });
  });

  test("un campo sin script de tecleo no llama a forms_keystroke", async ({ page }) => {
    const nombre = page.getByLabel("nombre");
    await nombre.fill("Ander");
    await nombre.blur();
    expect(await calls(page, "forms_keystroke")).toHaveLength(0);
    expect((await calls(page, "forms_fill"))[0]?.args).toMatchObject({ values: { nombre: "Ander" } });
  });

  test("el botón con acción llama a forms_run_action y enseña el aviso del script", async ({ page }) => {
    await page.getByRole("button", { name: "enviar" }).click();
    expect((await calls(page, "forms_run_action"))[0]?.args).toMatchObject({ name: "enviar", trigger: "U" });
    await expect(page.getByText("Factura enviada (simulado)")).toBeVisible();
  });
});
