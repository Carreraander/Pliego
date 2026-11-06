import { expect, test, type Page } from "@playwright/test";

/**
 * Humo de la app completa en el navegador con el backend de Tauri simulado: arranca la `App` de verdad (sin
 * `/nav-demo`), abre un documento y recorre OCR y Comparar comprobando los comandos que se envían. No sustituye a la
 * prueba en Windows, pero habría cazado que la interfaz saliera en inglés o que una herramienta siguiera «fantasma».
 */
const DOC = "C:\\Docs\\escaneo.pdf";

/** `locked`: otros archivos protegidos (ruta → contraseña) que `password_status` reconoce, como el servicio real. */
async function installBackend(page: Page, opts: { protectedDoc?: boolean; locked?: Record<string, string>; english?: boolean; signed?: boolean; crashed?: boolean } = {}) {
  await page.addInitScript(([doc, protectedDoc, locked, english, signed, crashed]: [string, boolean, Record<string, string>, boolean, boolean, boolean]) => {
    const calls: { cmd: string; args: unknown }[] = [];
    const responses: Record<string, unknown> = {
      // Con `crashed` la app arranca sin archivos y la vez anterior se cerró de golpe con una pestaña abierta.
      startup_files: crashed ? [] : [doc],
      session_previous: crashed ? { crashed: true, docs: [{ path: doc, page: 2 }] } : { crashed: false, docs: [] },
      crash_report: "Pliego 0.1.0 · windows x86_64",
      outline_tree: [{ title: "Capítulo A", page: 0, children: [], top: 700 }, { title: "Capítulo B", page: 2, children: [] }],
      outline_save: 1,
      open_document: 1,
      document_info: { page_count: 3, title: "Escaneo", author: null, is_pdf: true },
      page_sizes: [{ width: 595, height: 842 }, { width: 595, height: 842 }, { width: 595, height: 842 }],
      outline: [],
      recents_list: [],
      ocr_languages: { installed: ["eng", "osd", "spa"], downloadable: ["eng", "fra", "osd", "spa"] },
      pick_pdf_files: ["C:\\Docs\\version-2.pdf"],
      pick_save_pdf: "C:\\Copias\\escaneo revisado",
      save_document_as: "C:\\Copias\\escaneo revisado.pdf",
      ocr_pdf: 11,
      compress_pdf: 14,
      crop_pages: 18,
      make_accessible: 19,
      properties_save: 1,
      file_facts: { size: 2048, modified_secs: 1_700_000_000 },
      check_accessibility: {
        pages: 3, tagged: true, language: null, title: null,
        issues: [{ code: "figure_without_alt", severity: "error", page: 1, xref: 40, message: "Figura sin texto alternativo", suggestion: "" }],
      },
      analyze_pdf: null,
      compare_pdfs: 12,
      ocr_download_language: 13,
      merge_pdfs: 15,
      insert_pages: 16,
      sign_pdf: 17,
      sign_list_certificates: [{ thumbprint: "AB12", subject: "Ana Prueba", issuer: "CA de prueba", not_after: "2027-01-01T00:00:00Z", has_private_key: true }],
      sign_validate: signed
        ? [{
            field: "Firma1", signer: "Ana Prueba", subject: "CN=Ana Prueba", signing_time: "2026-11-05T10:00:00Z", reason: null, location: null,
            integrity: true, covers_whole_document: true, modifications_allowed: true, has_dss: false, certification: null, level: "b-b",
            chain: "incomplete", has_timestamp: true, document_timestamp: null,
            problems: ["el sello de tiempo es ilegible", "la cadena de certificados está incompleta"], problem_keys: ["timestamp-unreadable", "chain-incomplete"],
          }]
        : [],
      // Ajustes guardados: con `english` la persona eligió «English» en Ajustes (el valor por defecto es español).
      get_settings: english ? { version: 2, general: { language: "en" } } : null,
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
  }, [DOC, opts.protectedDoc ?? false, opts.locked ?? {}, opts.english ?? false, opts.signed ?? false, opts.crashed ?? false] as [string, boolean, Record<string, string>, boolean, boolean, boolean]);
}

const calls = (page: Page, cmd: string) =>
  page.evaluate((c) => ((window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls).filter((x) => x.cmd === c), cmd);

async function openTool(page: Page, query: string, option: RegExp) {
  await page.keyboard.press("Control+k");
  const box = page.getByRole("combobox", { name: /Paleta de órdenes|Command palette/ });
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

  test("Recortar páginas manda los márgenes en puntos y las páginas elegidas", async ({ page }) => {
    await openTool(page, "recortar", /^Recortar páginas/);
    await page.getByLabel("Arriba").fill("20");
    await page.getByLabel("Izquierda").fill("0");
    await page.getByLabel(/^Páginas/).fill("2-3");
    await page.getByRole("button", { name: /Recortar/ }).last().click();
    await expect.poll(async () => (await calls(page, "crop_pages")).length).toBe(1);
    expect((await calls(page, "crop_pages"))[0]?.args).toMatchObject({
      path: DOC, pages: [1, 2], margins: { left: 0, top: 56.69, right: 28.35, bottom: 28.35 },
    });
  });

  test("Hacer accesible manda título, idioma y los textos alternativos de las figuras", async ({ page }) => {
    await openTool(page, "hacer accesible", /^Hacer accesible/);
    await expect(page.getByText("El documento ya tiene etiquetas")).toBeVisible();
    await page.getByRole("textbox", { name: "Título" }).fill("Escaneo de abril");
    await page.getByLabel("Figura 1 (página 2)").fill("Gráfico de ventas");
    await page.getByRole("button", { name: "Guardar copia accesible" }).click();
    await expect.poll(async () => (await calls(page, "make_accessible")).length).toBe(1);
    expect((await calls(page, "make_accessible"))[0]?.args).toMatchObject({
      path: DOC,
      options: { autotag: false, title: "Escaneo de abril", language: "es-ES", alt_texts: [[40, "Gráfico de ventas"]], tab_order: true },
    });
  });

  test("Propiedades guarda solo lo que cambió en el propio PDF", async ({ page }) => {
    await openTool(page, "propiedades", /^Propiedades del documento/);
    await expect(page.getByRole("textbox", { name: "Título" })).toHaveValue("Escaneo");
    await page.getByRole("textbox", { name: "Autor" }).fill("Ana Muñoz");
    await page.getByRole("button", { name: "Guardar cambios" }).click();
    await expect.poll(async () => (await calls(page, "properties_save")).length).toBe(1);
    expect((await calls(page, "properties_save"))[0]?.args).toMatchObject({ path: DOC, edit: { author: "Ana Muñoz" } });
    expect(Object.keys((await calls(page, "properties_save"))[0]!.args.edit as object)).toEqual(["author"]);
    await expect(page.getByRole("button", { name: "Deshacer" })).toBeVisible();
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

test.describe("registro de diagnóstico (backend simulado)", () => {
  test.use({ locale: "en-US" });

  test("Ajustes > Avanzado abre la carpeta de registros", async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.keyboard.press("Control+,");
    await page.getByTestId("settings").getByRole("button", { name: "Avanzado" }).click();
    await page.getByRole("button", { name: "Abrir carpeta de registros" }).click();
    await expect.poll(async () => (await calls(page, "open_logs_folder")).length).toBe(1);
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

test.describe("firmas con la app en inglés (backend simulado)", () => {
  test.use({ locale: "en-US" });

  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page, { english: true, signed: true });
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
  });

  test("«Validar firmas» no deja texto fijo en español", async ({ page }) => {
    await page.getByRole("button", { name: /View signatures/ }).click();
    await expect(page.getByText("The timestamp cannot be read")).toBeVisible();
    await expect(page.getByText("Trusted certificate chain").or(page.getByText("Intermediate certificates are missing"))).toBeVisible();
    // Nada de lo que muestra el panel puede llevar los textos en español del motor.
    const panel = await page.locator(".pl-sign-panel").innerText();
    expect(panel).not.toMatch(/sello de tiempo|cadena de certificados|ilegible/i);
  });

  test("firmar manda el idioma de la app y el sufijo de la copia en inglés", async ({ page }) => {
    await openTool(page, "digital signature", /Digital signature/);
    await page.getByRole("combobox", { name: "Certificate" }).selectOption("AB12");
    await page.getByRole("button", { name: "Sign and save a copy" }).click();
    const sent = (await calls(page, "sign_pdf"))[0]?.args as { options: Record<string, unknown>; suffix: string };
    expect(sent.options).toMatchObject({ lang: "en" });
    expect(sent.suffix).toBe("signed");
  });
});

test.describe("atajos del visor y del navegador (backend simulado)", () => {
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await expect(page.getByText("Página 1 de 3")).toBeVisible();
  });

  test("Fin, Inicio, Ctrl+1 y Ctrl+W/Ctrl+Mayús+T hacen lo que dice la hoja de atajos", async ({ page }) => {
    await page.locator(".viewer").click({ position: { x: 5, y: 5 } });
    await page.keyboard.press("End");
    await expect(page.getByText("Página 3 de 3")).toBeVisible();
    await page.keyboard.press("Home");
    await expect(page.getByText("Página 1 de 3")).toBeVisible();
    await page.keyboard.press("Control+1");
    await expect(page.getByText("100 %", { exact: true })).toBeVisible();
    await page.keyboard.press("Control+w");
    await expect(page.getByText("Página 1 de 3")).toHaveCount(0);
    await page.keyboard.press("Control+Shift+T");
    await expect(page.getByText("Página 1 de 3")).toBeVisible();
    expect(await calls(page, "open_document")).toHaveLength(2);
  });

  test("Ctrl+F lleva a la caja de búsqueda y Ctrl+P no imprime la interfaz", async ({ page }) => {
    await page.keyboard.press("Control+f");
    await expect(page.getByRole("searchbox", { name: "Buscar" })).toBeFocused();
    await page.evaluate(() => { (window as unknown as { print: () => void; __printed: boolean }).print = () => { (window as unknown as { __printed: boolean }).__printed = true; }; });
    await page.keyboard.press("Control+p");
    await expect(page.getByText("La impresión llegará en una próxima versión.")).toBeVisible();
    expect(await page.evaluate(() => (window as unknown as { __printed?: boolean }).__printed)).toBeUndefined();
  });

  test("Ctrl+Mayús+S guarda una copia y pasa a trabajar con ella; Ctrl+S confirma que ya está guardado", async ({ page }) => {
    await page.keyboard.press("Control+s");
    await expect(page.getByText("Los cambios ya están guardados en el archivo.")).toBeVisible();
    await page.keyboard.press("Control+Shift+S");
    await expect(page.getByText("Guardado como escaneo revisado.pdf")).toBeVisible();
    expect((await calls(page, "pick_save_pdf"))[0]?.args).toMatchObject({ defaultName: "escaneo.pdf" });
    expect((await calls(page, "save_document_as"))[0]?.args).toMatchObject({ path: DOC, target: "C:\\Copias\\escaneo revisado" });
    const opened = await calls(page, "open_document");
    expect(opened.at(-1)?.args).toMatchObject({ path: "C:\\Copias\\escaneo revisado.pdf" });
    expect(await calls(page, "close_document")).toHaveLength(1);
  });

  test("F5, Ctrl+R y el menú contextual del navegador quedan bloqueados", async ({ page }) => {
    await page.evaluate(() => { (window as unknown as { __sinRecargar: boolean }).__sinRecargar = true; });
    const prevented = await page.evaluate(() => {
      const press = (key: string, ctrlKey = false) => {
        const e = new KeyboardEvent("keydown", { key, ctrlKey, bubbles: true, cancelable: true });
        document.body.dispatchEvent(e);
        return e.defaultPrevented;
      };
      const menu = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
      document.querySelector(".viewer")!.dispatchEvent(menu);
      return { f5: press("F5"), ctrlR: press("r", true), ctrlN: press("n", true), copy: press("c", true), menu: menu.defaultPrevented };
    });
    expect(prevented).toEqual({ f5: true, ctrlR: true, ctrlN: true, copy: false, menu: true });
    await page.keyboard.press("F5");
    await page.waitForTimeout(300);
    expect(await page.evaluate(() => (window as unknown as { __sinRecargar?: boolean }).__sinRecargar)).toBe(true);
  });
});

test.describe("recuperación tras un cierre inesperado (backend simulado)", () => {
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page, { crashed: true });
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
  });

  test("Inicio ofrece reabrir las pestañas y vuelve a la página en la que estaba", async ({ page }) => {
    const banner = page.getByRole("alert").filter({ hasText: "se cerró de forma inesperada" });
    await expect(banner).toContainText("1 documento abierto");
    await expect(banner.getByRole("button", { name: "Copiar informe" })).toBeVisible();
    await banner.getByRole("button", { name: "Reabrir" }).click();
    await expect(page.getByText("Página 3 de 3")).toBeVisible();
    expect((await calls(page, "open_document"))[0]?.args).toMatchObject({ path: DOC });
    await expect.poll(async () => (await calls(page, "session_save")).at(-1)?.args).toMatchObject({ docs: [{ path: DOC, page: 2 }] });
  });
});

test.describe("marcadores editables (backend simulado)", () => {
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.getByRole("button", { name: "Marcadores" }).first().click();
    await expect(page.getByRole("tree", { name: "Marcadores" })).toBeVisible();
  });

  test("mover, añadir y renombrar guardan el árbol entero sin perder la altura del destino", async ({ page }) => {
    await page.getByRole("button", { name: "Capítulo B" }).click();
    await page.keyboard.press("Alt+ArrowUp");
    await expect.poll(async () => (await calls(page, "outline_save")).length).toBe(1);
    const moved = (await calls(page, "outline_save"))[0]!.args as { nodes: { title: string; top?: number }[]; suffix: string };
    expect(moved.nodes.map((n) => n.title)).toEqual(["Capítulo B", "Capítulo A"]);
    expect(moved.nodes[1]).toMatchObject({ top: 700 });
    expect(moved.suffix).toBe("antes de los marcadores");

    await page.getByRole("button", { name: "Añadir un marcador a la página actual" }).click();
    const input = page.getByRole("textbox", { name: "Cambiar el nombre" });
    await expect(input).toBeFocused();
    await input.fill("Anexo");
    await input.press("Enter");
    await expect.poll(async () => (await calls(page, "outline_save")).length).toBe(3);
    const last = (await calls(page, "outline_save"))[2]!.args as { nodes: { title: string; page: number }[] };
    expect(last.nodes.map((n) => n.title)).toEqual(["Capítulo B", "Anexo", "Capítulo A"]);
    expect(last.nodes[1]).toMatchObject({ page: 2 }); // la página que se estaba viendo (la de «Capítulo B»)
    await expect(page.getByRole("button", { name: "Deshacer" }).first()).toBeEnabled();
  });
});
