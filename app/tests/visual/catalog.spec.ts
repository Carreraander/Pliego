import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test.describe("catálogo de componentes", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/ui-catalog");
    await page.waitForSelector(".catalog");
    await page.addStyleTag({ content: "*{caret-color:transparent !important} html,body,#root,.catalog{height:auto !important;overflow:visible !important}" });
  });

  test("captura completa", async ({ page }) => {
    await expect(page).toHaveScreenshot("catalogo.png", { fullPage: true });
  });

  test("sin infracciones graves de accesibilidad", async ({ page }) => {
    const results = await new AxeBuilder({ page }).analyze();
    const graves = results.violations.filter((v) => v.impact === "serious" || v.impact === "critical");
    expect(graves, JSON.stringify(graves.map((v) => ({ id: v.id, nodes: v.nodes.map((n) => n.target) })), null, 2)).toEqual([]);
  });

  test("el foco se ve al navegar con teclado", async ({ page }) => {
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    const outline = await page.evaluate(() => getComputedStyle(document.activeElement!).outlineStyle);
    expect(outline).not.toBe("none");
  });
});
