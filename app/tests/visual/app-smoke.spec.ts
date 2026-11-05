import { expect, test, type Page } from "@playwright/test";

/**
 * Humo de la app completa en el navegador con el backend de Tauri simulado: arranca la `App` de verdad (sin
 * `/nav-demo`), abre un documento y recorre OCR y Comparar comprobando los comandos que se envían. No sustituye a la
 * prueba en Windows, pero habría cazado que la interfaz saliera en inglés o que una herramienta siguiera «fantasma».
 */
const DOC = "C:\\Docs\\escaneo.pdf";

/** `locked`: otros archivos protegidos (ruta → contraseña) que `password_status` reconoce, como el servicio real. */
async function installBackend(page: Page, opts: { protectedDoc?: boolean; locked?: Record<string, string> } = {}) {
  await page.addInitScript(([doc, protectedDoc, locked]: [string, boolean, Record<string, string>]) => {
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
      compress_pdf: 14,
      analyze_pdf: null,
      compare_pdfs: 12,
      ocr_download_language: 13,
      merge_pdfs: 15,
      insert_pages: 16,
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
        // Documento con contraseña «abre»: sin ella (o con otra) el motor responde «password», como el servicio real.
        if (cmd === "open_document" && protectedDoc && (args as { password?: string }).password !== "abre") {
          throw { code: "password", message: "hace falta la contraseña" };
        }
        if (cmd === "password_status") {
          const { path, password } = args as { path: string; password: string | null };
          const want = locked[path] ?? (protectedDoc && path === doc ? "abre" : undefined);
          if (want === undefined || password === want) return "open";
          return password ? "wrong" : "required";
        }
        if (cmd === "plugin:event|listen") return nextId++;
        if (cmd.startsWith("plugin:")) return null;
        return cmd in responses ? responses[cmd] : null;
      },
    };
  }, [DOC, opts.protectedDoc ?? false, opts.locked ?? {}] as [string, boolean, Record<string, string>]);
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

test.describe("documento con contraseña (backend simulado)", () => {
  test.use({ locale: "en-US" });

  test("los trabajos sobre un PDF protegido reutilizan la contraseña que se escribió al abrirlo", async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page, { protectedDoc: true });
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.getByLabel("Contraseña").fill("abre");
    await page.getByRole("button", { name: "Abrir", exact: true }).click();
    await expect.poll(async () => (await calls(page, "open_document")).length).toBe(2);
    expect((await calls(page, "open_document"))[1]?.args).toMatchObject({ path: DOC, password: "abre" });

    await openTool(page, "comprimir", /^Comprimir/);
    await page.getByRole("button", { name: /Guardar copia/ }).click();
    expect((await calls(page, "compress_pdf"))[0]?.args).toMatchObject({ path: DOC, password: "abre" });
  });
});

const OTHER = "C:\\Docs\\version-2.pdf";

test.describe("otros archivos protegidos (backend simulado)", () => {
  test.use({ locale: "en-US" });
  test.beforeEach(({ browserName }, info) => {
    void browserName;
    test.skip(info.project.name !== "claro", "una sola variante basta");
  });

  async function openProtected(page: Page) {
    await installBackend(page, { protectedDoc: true, locked: { [OTHER]: "clave-b" } });
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.getByLabel("Contraseña").fill("abre");
    await page.getByRole("button", { name: "Abrir", exact: true }).click();
    await expect.poll(async () => (await calls(page, "open_document")).length).toBe(2);
  }

  test("Comparar pide la contraseña del segundo archivo y la manda como passwordB", async ({ page }) => {
    await openProtected(page);
    await openTool(page, "comparar", /^Comparar/);
    await page.getByRole("button", { name: "Elegir el otro PDF…" }).click();
    const dialog = page.getByRole("dialog", { name: "Archivo protegido" });
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText("version-2.pdf");
    await dialog.getByLabel("Contraseña").fill("mala");
    await dialog.getByRole("button", { name: "Abrir", exact: true }).click();
    await expect(dialog.getByText("La contraseña no es correcta")).toBeVisible();
    await dialog.getByLabel("Contraseña").fill("clave-b");
    await dialog.getByRole("button", { name: "Abrir", exact: true }).click();
    await expect(dialog).toBeHidden();
    await page.getByRole("button", { name: "Comparar y guardar informe" }).click();
    expect((await calls(page, "compare_pdfs"))[0]?.args).toMatchObject({
      path: DOC, other: OTHER, passwordA: "abre", passwordB: "clave-b", text: true, visual: true,
    });
  });

  test("si se cancela la contraseña del otro archivo, no se elige y no se puede comparar", async ({ page }) => {
    await openProtected(page);
    await openTool(page, "comparar", /^Comparar/);
    await page.getByRole("button", { name: "Elegir el otro PDF…" }).click();
    await page.getByRole("dialog", { name: "Archivo protegido" }).getByRole("button", { name: "Cancelar" }).click();
    await expect(page.getByRole("button", { name: "Comparar y guardar informe" })).toBeDisabled();
    expect(await calls(page, "compare_pdfs")).toHaveLength(0);
  });

  test("Reconocer texto sobre el PDF protegido manda la contraseña con la petición", async ({ page }) => {
    await openProtected(page);
    await openTool(page, "reconocer texto", /Reconocer texto/);
    await page.getByRole("button", { name: "Reconocer y guardar copia" }).click();
    expect((await calls(page, "ocr_pdf"))[0]?.args).toMatchObject({ request: { path: DOC, password: "abre" } });
  });

  test("Combinar manda la contraseña de cada archivo protegido", async ({ page }) => {
    await openProtected(page);
    await openTool(page, "combinar", /^Combinar/);
    await page.getByRole("button", { name: /Añadir/ }).click();
    const dialog = page.getByRole("dialog", { name: "Archivo protegido" });
    await dialog.getByLabel("Contraseña").fill("clave-b");
    await dialog.getByRole("button", { name: "Abrir", exact: true }).click();
    await page.getByRole("button", { name: /^Combinar/ }).last().click();
    expect((await calls(page, "merge_pdfs"))[0]?.args).toMatchObject({
      paths: [DOC, OTHER],
      options: { file_passwords: { [DOC]: "abre", [OTHER]: "clave-b" } },
    });
  });
});

