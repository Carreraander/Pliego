import { expect, test, type Page } from "@playwright/test";

/**
 * Modo Editar (W43) con el backend de Tauri simulado: comprueba que cada herramienta (texto con recolocación,
 * imágenes, objetos y enlaces) muestra lo que devuelve el servicio y envía el comando esperado con las coordenadas
 * correctas. La prueba con PDF reales está en `pliego-edit` y en acr12.
 */
const DOC = "C:\\Docs\\informe.pdf";
const OUTCOME = { undo_depth: 1, warnings: [] };

async function installBackend(page: Page) {
  await page.addInitScript(([doc, outcome]: [string, unknown]) => {
    const calls: { cmd: string; args: unknown }[] = [];
    const responses: Record<string, unknown> = {
      startup_files: [doc],
      open_document: 1,
      document_info: { page_count: 2, title: "Informe", author: null, is_pdf: true },
      page_sizes: [{ width: 595, height: 842 }, { width: 595, height: 842 }],
      outline: [],
      recents_list: [],
      pick_image_file: "C:\\Docs\\logo.png",
      edit_text_blocks: [{ page: 0, id: 0, bbox: [72, 100, 300, 140], text: "Hola mundo", lines: ["Hola mundo"], font_name: "Helvetica", font_size: 12, color: [0, 0, 0], bold: false, italic: false, family: "sans" }],
      edit_images: [{ page: 0, id: 0, name: "Im0", xref: 7, bbox: [100, 200, 200, 250] }],
      edit_vectors: [{ page: 0, id: 3, bbox: [50, 300, 150, 301] }],
      edit_links: [{ page: 0, id: 4, rect: [72, 400, 200, 420], target: { type: "uri", uri: "https://pdfpliego.com" } }],
      edit_text_replace: outcome, edit_image_insert: outcome, edit_image_replace: outcome, edit_image_crop: outcome,
      edit_object_transform: outcome, edit_object_delete: outcome, edit_link_add: outcome, edit_link_update: outcome, edit_link_delete: outcome,
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
  }, [DOC, OUTCOME] as [string, unknown]);
}

const calls = (page: Page, cmd: string) =>
  page.evaluate((c) => ((window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls).filter((x) => x.cmd === c), cmd);

test.describe("modo Editar (backend simulado)", () => {
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.locator(".pl-modebar__modes").getByRole("tab", { name: "Editar" }).click();
  });

  test("el texto se edita con recolocación activada por defecto y con color", async ({ page }) => {
    await page.getByRole("button", { name: /Párrafo: Hola mundo/ }).first().click();
    await page.getByLabel("Texto", { exact: true }).fill("Hola mundo, ahora mucho más largo");
    await page.getByRole("button", { name: "Aplicar" }).click();
    const sent = (await calls(page, "edit_text_replace"))[0]?.args as Record<string, unknown>;
    expect(sent).toMatchObject({ page: 0, block: 0, text: "Hola mundo, ahora mucho más largo", flow: true });
  });

  test("desmarcar «Recolocar lo de debajo» lo envía como flow = false", async ({ page }) => {
    await page.getByRole("checkbox", { name: "Recolocar lo de debajo" }).uncheck();
    await page.getByRole("button", { name: /Párrafo: Hola mundo/ }).first().click();
    await page.getByLabel("Texto", { exact: true }).fill("Otro texto");
    await page.getByRole("button", { name: "Aplicar" }).click();
    expect(((await calls(page, "edit_text_replace"))[0]?.args as Record<string, unknown>).flow).toBe(false);
  });

  test("imágenes: se elige, se arrastra y se envía la transformación; también se borra", async ({ page }) => {
    await page.getByRole("radio", { name: "Imágenes" }).check();
    const img = page.getByRole("button", { name: "Imagen 1" }).first();
    await expect(img).toBeVisible();
    const box = (await img.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 30, box.y + box.height / 2 + 20, { steps: 4 });
    await page.mouse.up();
    const t = (await calls(page, "edit_object_transform"))[0]?.args as { kind: string; id: number; transform: { dx: number; dy: number; scale_x: number } };
    expect(t).toMatchObject({ kind: "image", id: 0, page: 0 });
    expect(Math.abs(t.transform.dx)).toBeGreaterThan(1);
    expect(t.transform.scale_x).toBeCloseTo(1);
    await page.getByRole("button", { name: "Borrar", exact: true }).click();
    expect((await calls(page, "edit_object_delete"))[0]?.args).toMatchObject({ kind: "image", id: 0, page: 0 });
  });

  test("imágenes: reemplazar pide el archivo y envía su ruta", async ({ page }) => {
    await page.getByRole("radio", { name: "Imágenes" }).check();
    await page.getByRole("button", { name: "Imagen 1" }).first().click();
    await page.getByRole("button", { name: "Reemplazar imagen" }).click();
    expect((await calls(page, "edit_image_replace"))[0]?.args).toMatchObject({ page: 0, id: 0, image: "C:\\Docs\\logo.png" });
  });

  test("objetos vectoriales: se eligen y se giran", async ({ page }) => {
    await page.getByRole("radio", { name: "Objetos" }).check();
    await page.getByRole("button", { name: "Objeto 4" }).first().click();
    await page.getByRole("button", { name: "Girar 90°" }).click();
    expect((await calls(page, "edit_object_transform"))[0]?.args).toMatchObject({ kind: "vector", id: 3, transform: { rotate_degrees: 90 } });
  });

  test("enlaces: cambiar el destino a una página y borrar", async ({ page }) => {
    await page.getByRole("radio", { name: "Enlaces" }).check();
    await page.getByRole("button", { name: /Enlace a https:\/\/pdfpliego.com/ }).first().click();
    const field = page.getByLabel("Destino del enlace");
    await expect(field).toHaveValue("https://pdfpliego.com");
    await field.fill("2");
    await page.getByRole("button", { name: "Aplicar" }).click();
    expect((await calls(page, "edit_link_update"))[0]?.args).toMatchObject({ page: 0, id: 4, target: { type: "page", page: 1 } });

    await page.getByRole("button", { name: /Enlace a https:\/\/pdfpliego.com/ }).first().click();
    await page.getByRole("button", { name: "Borrar enlace" }).click();
    expect((await calls(page, "edit_link_delete"))[0]?.args).toMatchObject({ page: 0, id: 4 });
  });

  test("enlaces: un destino no válido no se puede aplicar", async ({ page }) => {
    await page.getByRole("radio", { name: "Enlaces" }).check();
    await page.getByRole("button", { name: /Enlace a https:\/\/pdfpliego.com/ }).first().click();
    await page.getByLabel("Destino del enlace").fill("99");
    await expect(page.getByRole("button", { name: "Aplicar" })).toBeDisabled();
  });
});
