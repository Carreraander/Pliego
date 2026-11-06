import { expect, test, type Page } from "@playwright/test";

/**
 * Modo Editar (W43) con el backend de Tauri simulado: comprueba que cada herramienta (texto con recolocación,
 * imágenes, objetos y enlaces) muestra lo que devuelve el servicio y envía el comando esperado con las coordenadas
 * correctas. La prueba con PDF reales está en `pliego-edit` y en acr12.
 */
const DOC = "C:\\Docs\\informe.pdf";
const OUTCOME = { undo_depth: 1, warnings: [] };

async function installBackend(page: Page, signed = false) {
  await page.addInitScript(([doc, outcome, isSigned]: [string, unknown, boolean]) => {
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
      edit_is_signed: isSigned,
      edit_text_replace: outcome, edit_image_insert: outcome, edit_image_replace: outcome, edit_image_crop: outcome,
      edit_object_transform: outcome, edit_object_delete: outcome, edit_link_add: outcome, edit_link_update: outcome, edit_link_delete: outcome,
    };
    let nextId = 1;
    const w = window as unknown as Record<string, unknown>;
    w.__calls = calls;
    w.__responses = responses;
    // Eventos del backend: `__emit("document-changed", {...})` llama a lo que registró `listen`.
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
      invoke: async (cmd: string, args: unknown) => {
        calls.push({ cmd, args });
        if (cmd === "plugin:event|listen") {
          const a = args as { event: string; handler: number };
          listeners[a.event] = a.handler;
          return nextId++;
        }
        if (cmd.startsWith("plugin:")) return null;
        return cmd in responses ? responses[cmd] : null;
      },
    };
  }, [DOC, OUTCOME, signed] as [string, unknown, boolean]);
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
    await expect.poll(async () => (await calls(page, "edit_object_delete"))[0]?.args).toMatchObject({ kind: "image", id: 0, page: 0 });
  });

  test("imágenes: reemplazar pide el archivo y envía su ruta", async ({ page }) => {
    await page.getByRole("radio", { name: "Imágenes" }).check();
    await page.getByRole("button", { name: "Imagen 1" }).first().click();
    await page.getByRole("button", { name: "Reemplazar imagen" }).click();
    await expect.poll(async () => (await calls(page, "edit_image_replace"))[0]?.args).toMatchObject({ page: 0, id: 0, image: "C:\\Docs\\logo.png" });
  });

  test("objetos vectoriales: se eligen y se giran", async ({ page }) => {
    await page.getByRole("radio", { name: "Objetos" }).check();
    await page.getByRole("button", { name: "Objeto 4" }).first().click();
    await page.getByRole("button", { name: "Girar 90°" }).click();
    await expect.poll(async () => (await calls(page, "edit_object_transform"))[0]?.args).toMatchObject({ kind: "vector", id: 3, transform: { rotate_degrees: 90 } });
  });

  test("enlaces: cambiar el destino a una página y borrar", async ({ page }) => {
    await page.getByRole("radio", { name: "Enlaces" }).check();
    await page.getByRole("button", { name: /Enlace a https:\/\/pdfpliego.com/ }).first().click();
    const field = page.getByLabel("Destino del enlace");
    await expect(field).toHaveValue("https://pdfpliego.com");
    await field.fill("2");
    await page.getByRole("button", { name: "Aplicar" }).click();
    await expect.poll(async () => (await calls(page, "edit_link_update"))[0]?.args).toMatchObject({ page: 0, id: 4, target: { type: "page", page: 1 } });

    await page.getByRole("button", { name: /Enlace a https:\/\/pdfpliego.com/ }).first().click();
    await page.getByRole("button", { name: "Borrar enlace" }).click();
    await expect.poll(async () => (await calls(page, "edit_link_delete"))[0]?.args).toMatchObject({ page: 0, id: 4 });
  });

  test("enlaces: un destino no válido no se puede aplicar", async ({ page }) => {
    await page.getByRole("radio", { name: "Enlaces" }).check();
    await page.getByRole("button", { name: /Enlace a https:\/\/pdfpliego.com/ }).first().click();
    await page.getByLabel("Destino del enlace").fill("99");
    await expect(page.getByRole("button", { name: "Aplicar" })).toBeDisabled();
  });

  test("el visor conserva la posición al cambiar la revisión del documento (tras editar una imagen)", async ({ page }) => {
    await page.getByRole("radio", { name: "Imágenes" }).check();
    const viewer = page.getByTestId("viewer");
    await viewer.evaluate((el) => { el.scrollTop = 300; });
    await expect.poll(() => viewer.evaluate((el) => el.scrollTop)).toBe(300);
    // Deja pasar el evento de scroll (asíncrono) antes de cambiar la revisión.
    await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))));
    await page.evaluate(() => (window as unknown as { __emit: (e: string, p: unknown) => void }).__emit("document-changed", { doc_id: 1, revision: 5 }));
    await page.waitForTimeout(300);
    await expect.poll(() => page.getByTestId("viewer").evaluate((el) => el.scrollTop)).toBe(300);
  });

  test("si la edición cambia el número de páginas, el visor lo refleja sin reabrir", async ({ page }) => {
    await expect(page.getByText("de 2").first()).toBeVisible();
    await page.evaluate(() => {
      const w = window as unknown as { __responses: Record<string, unknown>; __emit: (e: string, p: unknown) => void };
      w.__responses.page_sizes = [{ width: 595, height: 842 }, { width: 595, height: 842 }, { width: 595, height: 842 }];
      w.__emit("document-changed", { doc_id: 1, revision: 6 });
    });
    await expect(page.getByText(/Página \d+ de 3/).first()).toBeVisible();
  });

  test("un documento firmado avisa antes de la primera edición y no edita si se cancela", async ({ page }) => {
    // Se vuelve a abrir con el backend que dice que el documento está firmado.
    const fresh = await page.context().newPage();
    await installBackend(fresh, true);
    await fresh.goto("/");
    await fresh.waitForSelector(".pl-window", { timeout: 45_000 });
    await fresh.locator(".pl-modebar__modes").getByRole("tab", { name: "Editar" }).click();
    const edit = async () => {
      await fresh.getByRole("button", { name: /Párrafo: Hola mundo/ }).first().click();
      await fresh.getByLabel("Texto", { exact: true }).fill("Texto distinto");
      await fresh.getByRole("button", { name: "Aplicar" }).click();
    };
    await edit();
    const dialog = fresh.getByRole("dialog", { name: "Documento firmado" });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Cancelar" }).click();
    expect(await calls(fresh, "edit_text_replace")).toHaveLength(0);
    await edit();
    await fresh.getByRole("dialog", { name: "Documento firmado" }).getByRole("button", { name: "Editar de todos modos" }).click();
    await expect.poll(async () => (await calls(fresh, "edit_text_replace")).length).toBe(1);
    // Una vez aceptado, no vuelve a preguntar en este documento.
    await edit();
    await expect.poll(async () => (await calls(fresh, "edit_text_replace")).length).toBe(2);
    await expect(fresh.getByRole("dialog", { name: "Documento firmado" })).toBeHidden();
  });

  test("imágenes: mientras se arrastra se ve una vista previa de la imagen en su nueva posición", async ({ page }) => {
    await page.getByRole("radio", { name: "Imágenes" }).check();
    const img = page.getByRole("button", { name: "Imagen 1" }).first();
    const box = (await img.boundingBox())!;
    await expect(page.getByTestId("edit-ghost")).toHaveCount(0);
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 40, box.y + box.height / 2 + 30, { steps: 4 });
    const ghost = page.getByTestId("edit-ghost");
    await expect(ghost).toHaveCount(1);
    await expect(ghost.locator("img").first()).toBeAttached();
    // Mover no cambia el tamaño: no se escala.
    const [g, o] = [await ghost.boundingBox(), await img.boundingBox()];
    expect(Math.abs(g!.x - o!.x)).toBeLessThan(3);
    expect(Math.abs(g!.y - o!.y)).toBeLessThan(3);
    expect(Math.abs(g!.width - o!.width)).toBeLessThan(3);
    await expect(ghost.locator(".pl-edit-ghost__page")).toHaveCSS("transform", /matrix\(1, 0, 0, 1,/);
    await page.mouse.up();
    await expect(page.getByTestId("edit-ghost")).toHaveCount(0);
    expect((await calls(page, "edit_object_transform")).length).toBe(1);
  });
});
