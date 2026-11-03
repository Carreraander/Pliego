import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test.describe("armazón", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/ui-shell");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await page.addStyleTag({ content: "*{caret-color:transparent !important}" });
  });

  test("captura 1280 px", async ({ page }) => {
    await expect(page).toHaveScreenshot("armazon-1280.png");
  });

  test("captura 1000 px (modos plegados)", async ({ page }) => {
    await page.setViewportSize({ width: 1000, height: 700 });
    await expect(page).toHaveScreenshot("armazon-1000.png");
  });

  test("captura 800 px (paneles superpuestos)", async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 600 });
    await page.getByRole("button", { name: "Miniaturas" }).click();
    await expect(page).toHaveScreenshot("armazon-800.png");
  });

  test("sin infracciones graves de accesibilidad", async ({ page }) => {
    await page.getByRole("button", { name: "Miniaturas" }).click();
    const results = await new AxeBuilder({ page }).analyze();
    const graves = results.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
    expect(graves, JSON.stringify(graves.map((v) => ({ id: v.id, nodes: v.nodes.map((n) => n.html.slice(0, 120)) })), null, 2)).toEqual([]);
  });

  test("F6 recorre las regiones en el orden del diseño", async ({ page }) => {
    await page.getByRole("tab", { name: /contrato/ }).focus();
    const region = () => page.evaluate(() => document.activeElement?.closest<HTMLElement>("[data-region]")?.dataset.region);
    expect(await region()).toBe("titlebar");
    await page.keyboard.press("F6"); expect(await region()).toBe("modebar");
    await page.keyboard.press("F6"); expect(await region()).toBe("rail");
    await page.keyboard.press("F6"); expect(await region()).toBe("canvas");
    await page.keyboard.press("F6"); expect(await region()).toBe("inspector");
    await page.keyboard.press("F6"); expect(await region()).toBe("statusbar");
    await page.keyboard.press("Shift+F6"); expect(await region()).toBe("inspector");
  });

  test("modos: Alt+n cambia de modo y las flechas se mueven por la barra", async ({ page }) => {
    await page.keyboard.press("Alt+3");
    await expect(page.getByRole("tab", { name: "Rellenar y firmar" })).toHaveAttribute("aria-selected", "true");
    await page.getByRole("tab", { name: "Leer" }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(page.getByRole("tab", { name: "Comentar" })).toBeFocused();
  });

  test("raíl: abre y pliega el panel lateral, F4 lo alterna y el ancho se puede cambiar con teclado", async ({ page }) => {
    const rail = page.getByRole("button", { name: "Miniaturas" });
    await rail.click();
    await expect(page.getByRole("complementary", { name: "Miniaturas" })).toBeVisible();
    await expect(rail).toHaveAttribute("aria-pressed", "true");
    const sep = page.getByRole("separator", { name: "Ancho del panel lateral" });
    await sep.focus();
    const before = Number(await sep.getAttribute("aria-valuenow"));
    await page.keyboard.press("ArrowRight");
    expect(Number(await sep.getAttribute("aria-valuenow"))).toBe(before + 16);
    await rail.click();
    await expect(page.getByRole("complementary", { name: "Miniaturas" })).toBeHidden();
    await page.keyboard.press("F4");
    await expect(page.getByRole("complementary", { name: "Miniaturas" })).toBeVisible();
  });

  test("pestañas: Ctrl+Mayús+→ reordena y Supr cierra", async ({ page }) => {
    const first = page.getByRole("tab", { name: /contrato/ });
    await first.focus();
    await page.keyboard.press("Control+Shift+ArrowRight");
    const names = await page.getByRole("tab").filter({ hasText: /pdf/ }).allInnerTexts();
    expect(names[0]).toContain("factura");
    await page.getByRole("tab", { name: /factura/ }).focus();
    await page.keyboard.press("Delete");
    await expect(page.getByRole("tab", { name: /factura/ })).toHaveCount(0);
  });

  test("F9 oculta y recupera el inspector", async ({ page }) => {
    await expect(page.getByRole("complementary", { name: "Inspector" })).toBeVisible();
    await page.keyboard.press("F9");
    await expect(page.getByRole("complementary", { name: "Inspector" })).toBeHidden();
    await page.keyboard.press("F9");
    await expect(page.getByRole("complementary", { name: "Inspector" })).toBeVisible();
  });

  test("una pestaña de tipo pantalla oculta la barra de modos y «Pág.»", async ({ page }) => {
    await expect(page.getByRole("tablist", { name: "Modos" })).toBeVisible();
    await expect(page.getByText("Pág. 2 de 48")).toBeVisible();
    await page.getByRole("tab", { name: "Inicio" }).click();
    await expect(page.getByRole("tablist", { name: "Modos" })).toBeHidden();
    await expect(page.getByText("Pág. 2 de 48")).toBeHidden();
    await expect(page.getByText("Firmado · válido")).toBeVisible();
    await expect(page.getByRole("button", { name: /Buscar o ejecutar/ })).toBeVisible();
    await expect(page).toHaveScreenshot("armazon-inicio.png");
  });
});
