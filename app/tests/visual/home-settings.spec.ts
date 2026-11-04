import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

async function open(page: import("@playwright/test").Page, screen: "home" | "settings") {
  await page.goto("/nav-demo");
  await page.waitForSelector(".pl-window");
  await page.waitForFunction(() => "__demo" in window);
  await page.evaluate((s) => (window as unknown as { __demo: Record<string, () => void> }).__demo[s]!(), screen);
  await page.addStyleTag({ content: "*{caret-color:transparent !important}" });
}
const graves = async (page: import("@playwright/test").Page) => {
  const results = await new AxeBuilder({ page }).analyze();
  return results.violations.filter((v) => v.impact === "serious" || v.impact === "critical").map((v) => ({ id: v.id, nodes: v.nodes.map((n) => n.html.slice(0, 140)) }));
};

test.describe("Inicio", () => {
  test("muestra saludo, acciones rápidas y recientes con fijado y archivo desaparecido", async ({ page }) => {
    await open(page, "home");
    const home = page.getByTestId("home");
    await expect(home.getByRole("heading", { level: 2 })).toContainText(/Buenos días|Buenas tardes|Buenas noches/);
    await expect(home.getByRole("button", { name: /Suelta aquí un PDF/ })).toBeVisible();
    for (const name of ["Abrir", "Combinar", "Reconocer texto", "Firmar", "Comprimir", "Convertir"]) await expect(home.locator(".pl-qa", { hasText: name }).first()).toBeVisible();
    const rows = home.locator(".pl-recent");
    await expect(rows).toHaveCount(3);
    await expect(rows.first()).toContainText("contrato.pdf");
    await expect(rows.first()).toContainText("hoy,");
    await expect(rows.nth(1)).toContainText("ayer");
    await expect(rows.nth(2)).toHaveClass(/pl-recent--missing/);
    await expect(rows.nth(2)).toContainText("No se encuentra");
    await expect(home).toContainText("AGPL-3.0");
    await expect(page.locator(".pl-modebar")).toHaveCount(0);
  });
  test("fijar y quitar un reciente, y personalizar las acciones rápidas", async ({ page }) => {
    await open(page, "home");
    const home = page.getByTestId("home");
    await home.getByRole("button", { name: /Fijar arriba: factura-octubre/ }).click();
    await expect(home.locator(".pl-recent").first()).toContainText("contrato.pdf");
    await home.locator(".pl-recent").nth(1).click({ button: "right" });
    await page.getByRole("menuitem", { name: "Quitar de la lista" }).click();
    await expect(home.locator(".pl-recent")).toHaveCount(2);
    await home.getByRole("button", { name: "Personalizar" }).click();
    await page.getByRole("menuitemcheckbox", { name: "Dividir" }).click();
    await expect(home.locator(".pl-qa", { hasText: "Dividir" })).toBeVisible();
  });
  test("una acción rápida abre la herramienta (Comprimir)", async ({ page }) => {
    await open(page, "home");
    await page.getByTestId("home").locator(".pl-qa", { hasText: "Comprimir" }).click();
    await expect(page.getByText("Ahora 12,4 MB")).toBeVisible();
  });
  test("capturas y accesibilidad", async ({ page }) => {
    await open(page, "home");
    await expect(page).toHaveScreenshot("inicio.png");
    expect(await graves(page)).toEqual([]);
  });
});

test.describe("Ajustes", () => {
  test("cambia el tema con efecto inmediato y se conserva", async ({ page }) => {
    await open(page, "settings");
    const settings = page.getByTestId("settings");
    await expect(page.locator(".pl-modebar")).toHaveCount(0);
    await expect(settings.getByRole("heading", { name: "General" })).toBeVisible();
    await settings.getByLabel("Tema").selectOption("dark");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
    await page.reload();
    await page.waitForFunction(() => "__demo" in window);
    await page.evaluate(() => (window as unknown as { __demo: Record<string, () => void> }).__demo.settings!());
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });
  test("el buscador de ajustes filtra filas y secciones", async ({ page }) => {
    await open(page, "settings");
    const settings = page.getByTestId("settings");
    await settings.getByRole("searchbox", { name: "Buscar ajustes" }).fill("nocturna");
    await expect(settings.getByText("Lectura nocturna")).toBeVisible();
    await expect(settings.getByText("Idioma")).toBeHidden();
    await settings.getByRole("searchbox", { name: "Buscar ajustes" }).fill("zzzz");
    await expect(settings.getByText(/Ningún ajuste coincide/)).toBeVisible();
  });
  test("Actualizaciones usa el servicio simulado y Acerca de enseña la licencia y el código fuente", async ({ page }) => {
    await open(page, "settings");
    const settings = page.getByTestId("settings");
    await settings.getByRole("button", { name: "Actualizaciones" }).click();
    await settings.getByRole("button", { name: "Buscar actualizaciones" }).click();
    await expect(settings.getByText("Pliego está al día").first()).toBeVisible();
    await expect(settings.getByText(/Servicio simulado/)).toBeVisible();
    await settings.getByRole("button", { name: "Acerca de" }).click();
    await expect(settings).toContainText("AGPL-3.0-or-later");
    await expect(settings.getByRole("link", { name: /github.com\/Carreraander\/Pliego/ })).toBeVisible();
  });
  test("Avanzado abre la hoja de atajos; Ctrl+K encuentra un ajuste", async ({ page }) => {
    await open(page, "settings");
    await page.getByTestId("settings").getByRole("button", { name: "Avanzado" }).click();
    await page.getByRole("button", { name: /Ver y cambiar atajos/ }).click();
    await expect(page.getByRole("dialog", { name: "Atajos de teclado" })).toBeVisible();
    await page.keyboard.press("Escape");
    await page.keyboard.press("Control+k");
    await page.getByRole("combobox").fill("tema");
    await expect(page.getByRole("option", { name: /General › Tema/ })).toBeVisible();
  });
  test("capturas y accesibilidad", async ({ page }) => {
    await open(page, "settings");
    await expect(page).toHaveScreenshot("ajustes.png");
    expect(await graves(page)).toEqual([]);
  });
});

test.describe("Centro de tareas", () => {
  test("progreso, cancelar, abrir resultado y fallo con mensaje", async ({ page }) => {
    await page.goto("/nav-demo");
    await page.waitForFunction(() => "__demo" in window);
    await page.evaluate(() => {
      const d = (window as unknown as { __demo: Record<string, (...a: unknown[]) => void> }).__demo;
      d.track!(7, "Comprimir contrato.pdf"); d.progress!(7, 2, 4, "Página 2 de 4");
    });
    const status = page.getByRole("button", { name: /tarea/i });
    await expect(status).toContainText("1 tarea en curso");
    await status.click();
    const pop = page.getByRole("dialog", { name: "Tareas" });
    await expect(pop.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
    await expect(pop.getByRole("button", { name: "Cancelar" })).toBeVisible();
    await page.evaluate(() => (window as unknown as { __demo: Record<string, (...a: unknown[]) => void> }).__demo.finished!(7, true));
    await expect(pop.getByText("12,4 MB → 3,1 MB")).toBeVisible();
    await expect(pop.getByRole("button", { name: "Abrir resultado" })).toBeVisible();
    await expect(pop.getByRole("button", { name: "Mostrar en carpeta" })).toBeVisible();
    await page.evaluate(() => {
      const d = (window as unknown as { __demo: Record<string, (...a: unknown[]) => void> }).__demo;
      d.track!(8, "OCR de informe.pdf"); d.progress!(8, 0, 0, "Preparando…"); d.finished!(8, false);
    });
    await expect(pop.getByRole("alert")).toContainText("archivo dañado");
    await expect(page).toHaveScreenshot("centro-de-tareas.png");
  });
});
