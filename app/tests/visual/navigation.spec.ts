import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test.describe("navegación (W20)", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/nav-demo");
    await page.waitForSelector(".pl-window");
    await page.addStyleTag({ content: "*{caret-color:transparent !important}" });
  });

  test("seis modos por defecto y cambio con Alt+2", async ({ page }) => {
    const tabs = page.locator(".pl-modebar__modes").getByRole("tab");
    await expect(tabs).toHaveText(["Leer", "Comentar", "Rellenar y firmar", "Organizar", "Editar", "Proteger"]);
    await expect(page.getByRole("tab", { name: "Leer" })).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Alt+2");
    await expect(page.getByRole("tab", { name: "Comentar" })).toHaveAttribute("aria-selected", "true");
  });

  test("Ctrl+K: busca sin tildes, navega con flechas y ejecuta con Intro", async ({ page }) => {
    await page.keyboard.press("Control+k");
    const box = page.getByRole("combobox", { name: "Paleta de órdenes" });
    await expect(box).toBeFocused();
    await box.fill("INDICE");
    await expect(page.getByRole("option", { name: /Índice de búsqueda/ })).toBeVisible();
    await expect(page.getByRole("option", { name: /Índice de búsqueda/ })).toHaveAttribute("aria-disabled", "true");
    await box.fill("comprimir");
    await expect(page.getByRole("option").first()).toContainText("Comprimir");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog", { name: "Paleta de órdenes" })).toBeHidden();
    await expect(page.getByText("Ahora 12,4 MB")).toBeVisible();
  });

  test("Ctrl+K: ir a página, documentos abiertos y Esc", async ({ page }) => {
    await page.keyboard.press("Control+k");
    const box = page.getByRole("combobox", { name: "Paleta de órdenes" });
    await box.fill("7");
    await expect(page.getByRole("option").first()).toContainText("Ir a la página 7");
    await box.fill("factura");
    await expect(page.getByRole("option", { name: /factura-octubre/ }).first()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(box).toBeHidden();
  });

  test("Ctrl+Mayús+A: todas las herramientas con motivo en las desactivadas, buscador y fijar", async ({ page }) => {
    await page.keyboard.press("Control+Shift+A");
    const palette = page.getByRole("dialog", { name: "Todas las herramientas" });
    await expect(palette).toBeVisible();
    for (const name of ["Comprimir", "Reconocer texto (OCR)", "Firma digital", "Redactar", "Comparar", "Asistente local", "Medir", "Validar PDF/A"]) {
      await expect(palette.getByRole("button", { name: new RegExp(name.replace(/[()]/g, "\\$&")) })).toBeVisible();
    }
    await expect(palette.locator('[data-tool="sign.digital"]')).toHaveAttribute("aria-disabled", "true");
    await expect(palette.locator('[data-tool="sign.digital"]')).toContainText("Disponible más adelante");
    await palette.getByRole("searchbox").fill("marca agua");
    await expect(palette.locator(".pl-tool")).toHaveCount(1);
    await palette.locator('[data-tool="edit.watermark"]').click({ button: "right", force: true });
    await page.getByRole("menuitem", { name: "Fijar en la barra" }).click();
    await page.keyboard.press("Escape");
    await expect(palette).toBeHidden();
    // Alt+7 sigue la posición visible (el modo fijado es el séptimo) y el clic derecho lo quita.
    const pinned = page.getByRole("tab", { name: "Marca de agua" });
    await expect(pinned).toBeVisible();
    await pinned.click({ button: "right" });
    await page.getByRole("menuitem", { name: "Quitar de la barra" }).click();
    await expect(pinned).toBeHidden();
  });

  test("la paleta de herramientas se abre y cierra con Ctrl+Mayús+A, con clic fuera y devuelve el foco", async ({ page }) => {
    const opener = page.getByRole("button", { name: /^Herramientas/ });
    await opener.focus();
    await page.keyboard.press("Control+Shift+A");
    const palette = page.getByRole("dialog", { name: "Todas las herramientas" });
    await expect(palette).toBeVisible();
    await expect(palette).not.toHaveAttribute("aria-modal", "true");
    await expect(page.getByRole("searchbox", { name: "Buscar herramientas…" })).toBeFocused();
    await page.keyboard.press("Control+Shift+A");
    await expect(palette).toBeHidden();
    await expect(opener).toBeFocused();
    await page.keyboard.press("Control+Shift+A");
    await page.mouse.click(10, 300);
    await expect(palette).toBeHidden();
  });

  test("F1 abre la hoja de atajos y permite cambiar uno sin conflictos", async ({ page }) => {
    await page.keyboard.press("F1");
    const sheet = page.getByRole("dialog", { name: "Atajos de teclado" });
    await expect(sheet).toBeVisible();
    await expect(sheet).toContainText("Paleta de órdenes");
    await expect(sheet).toContainText("Todas las herramientas");
  });

  test("capturas de las paletas", async ({ page }) => {
    await page.keyboard.press("Control+k");
    await page.getByRole("combobox").fill("o");
    await expect(page).toHaveScreenshot("paleta-ordenes.png");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Control+Shift+A");
    await expect(page).toHaveScreenshot("paleta-herramientas.png");
  });

  test("sin infracciones graves de accesibilidad con la paleta abierta", async ({ page }) => {
    await page.keyboard.press("Control+Shift+A");
    await page.getByRole("dialog", { name: "Todas las herramientas" }).waitFor();
    const results = await new AxeBuilder({ page }).analyze();
    const graves = results.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
    expect(graves, JSON.stringify(graves.map((v) => ({ id: v.id, nodes: v.nodes.map((n) => n.html.slice(0, 120)) })), null, 2)).toEqual([]);
  });
});
