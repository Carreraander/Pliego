import { expect, test } from "@playwright/test";

test.describe("superposiciones del catálogo (teclado)", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/ui-catalog");
    await page.waitForSelector(".catalog", { timeout: 45_000 });
  });

  test("el menú se abre, navega con flechas, abre submenú y devuelve el foco", async ({ page }) => {
    await page.focus("#menu-btn");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menu", { name: "Menú de ejemplo" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Abrir…" })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("menuitem", { name: "Nuevo" })).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(page.getByRole("menuitem", { name: "Desde archivos" })).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await expect(page.getByRole("menuitem", { name: "Nuevo" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("menu", { name: "Menú de ejemplo" })).toBeHidden();
    await expect(page.locator("#menu-btn")).toBeFocused();
  });

  test("las entradas desactivadas se saltan y una casilla de menú conmuta", async ({ page }) => {
    await page.focus("#menu-btn");
    await page.keyboard.press("Enter");
    await page.getByRole("menuitemcheckbox", { name: "Ajustar líneas" }).click();
    await page.mouse.move(0, 0); // el menú reabierto no debe quedar bajo el puntero (el hover mueve el foco)
    await page.focus("#menu-btn");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menuitem", { name: "Abrir…" })).toBeFocused();
    await expect(page.getByRole("menuitemcheckbox", { name: "Ajustar líneas" })).toHaveAttribute("aria-checked", "false");
    await page.keyboard.press("End");
    await expect(page.getByRole("menuitem", { name: "Eliminar" })).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(page.getByRole("menuitemcheckbox", { name: "Ajustar líneas" })).toBeFocused();
  });

  test("el menú contextual se abre con clic derecho", async ({ page }) => {
    await page.click("#ctx-area", { button: "right" });
    await expect(page.getByRole("menu", { name: "Menú contextual" })).toBeVisible();
  });

  test("el tooltip aparece al enfocar con teclado, con su atajo", async ({ page }) => {
    await page.focus("#tip-btn");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Tab");
    const tip = page.getByRole("tooltip");
    await expect(tip).toBeVisible();
    await expect(tip).toContainText("Buscar");
    await expect(tip).toContainText("Ctrl");
    await page.keyboard.press("Escape");
    await expect(tip).toBeHidden();
  });

  test("las pestañas cambian con flechas y enlazan su panel", async ({ page }) => {
    await page.getByRole("tab", { name: "Propiedades" }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(page.getByRole("tab", { name: "Comentarios" })).toHaveAttribute("aria-selected", "true");
    await expect(page.getByRole("tabpanel", { name: "Comentarios" })).toContainText("comentarios");
  });

  test("los avisos emergentes se anuncian y se cierran", async ({ page }) => {
    await page.getByRole("button", { name: "Error", exact: true }).click();
    const t = page.getByRole("alert").filter({ hasText: "No se pudo guardar" });
    await expect(t).toBeVisible();
    await t.getByRole("button", { name: "Cerrar" }).click();
    await expect(t).toBeHidden();
  });
});

test("captura con menú, submenú y aviso abiertos", async ({ page }) => {
  await page.goto("/ui-catalog");
  await page.waitForSelector(".catalog", { timeout: 45_000 });
  await page.getByRole("button", { name: "Aviso", exact: true }).click();
  await page.getByRole("button", { name: "Error", exact: true }).click();
  await page.focus("#menu-btn");
  await page.keyboard.press("Enter");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowRight");
  await page.addStyleTag({ content: "*{caret-color:transparent !important}" });
  await expect(page).toHaveScreenshot("superposiciones.png");
});
