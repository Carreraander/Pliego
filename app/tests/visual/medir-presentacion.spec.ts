import { expect, test, type Page } from "@playwright/test";
import { deflateSync } from "node:zlib";

/**
 * Medir y Presentación con el backend de Tauri simulado (validación sin pantalla real, 2026-11-05). Las teselas son
 * PNG sintéticos (una cuadrícula con el número de página) y los comandos de medida los resuelve un simulador que
 * aplica la misma aritmética que `pliego-measure` (unidades por punto × distancia), de modo que se prueba la
 * geometría de la interfaz (zoom, clics, calibrar, medir, área) y no solo que se llame al comando.
 */
const DOC = "C:\\Docs\\plano.pdf";

function crc32(buf: Buffer): number {
  let c, crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c = (crc ^ buf[i]!) & 0xff;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}
const chunk = (type: string, data: Buffer) => {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
};
/** PNG gris de 256×256 con rejilla cada 32 px: no hace falta nada más para ver que la hoja se pinta. */
function tilePng(shade: number): Buffer {
  const w = 256, h = 256, raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 3 + 1)] = 0;
    for (let x = 0; x < w; x++) {
      const line = x % 32 === 0 || y % 32 === 0;
      const v = line ? 150 : shade;
      const o = y * (w * 3 + 1) + 1 + x * 3;
      raw[o] = v; raw[o + 1] = v; raw[o + 2] = v;
    }
  }
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

async function installBackend(page: Page) {
  // Con «Windows» en el agente de usuario la app pide las teselas a http://pliego.localhost, que sí se puede interceptar.
  await page.route("http://pliego.localhost/tile/**", async (route) => {
    const pageNo = Number(new URL(route.request().url()).pathname.split("/")[3] ?? 0);
    await route.fulfill({ status: 200, contentType: "image/png", body: tilePng(235 + (pageNo % 3) * 6) });
  });
  await page.addInitScript((doc: string) => {
    const calls: { cmd: string; args: Record<string, unknown> }[] = [];
    const sizes = Array.from({ length: 3 }, () => ({ width: 595, height: 842 }));
    let nextId = 1;
    const w = window as unknown as Record<string, unknown>;
    w.__calls = calls;
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
    const dist = (pts: number[][]) => pts.slice(1).reduce((s, p, i) => s + Math.hypot(p[0]! - pts[i]![0]!, p[1]! - pts[i]![1]!), 0);
    const area = (pts: number[][]) => Math.abs(pts.reduce((s, p, i) => { const q = pts[(i + 1) % pts.length]!; return s + p[0]! * q[1]! - q[0]! * p[1]!; }, 0)) / 2;
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
          case "document_info": return { page_count: 3, title: "Plano", author: null, is_pdf: true };
          case "page_sizes": return sizes;
          case "outline": case "recents_list": return [];
          case "measure_page_scale": return null;
          case "measure_calibrate": return { unit: args.unit, units_per_point: (args.real as number) / (args.pagePoints as number) };
          case "measure_ratio": return { unit: args.unit, units_per_point: (args.n as number) * 0.3527777778 / 1000 };
          case "measure_shape": {
            const s = args.scale as { unit: string; units_per_point: number };
            const pts = args.points as number[][];
            const closed = args.closed as boolean;
            const length = (closed ? dist([...pts, pts[0]!]) : dist(pts)) * s.units_per_point;
            const ar = closed ? area(pts) * s.units_per_point ** 2 : null;
            const text = ar === null ? `${length.toFixed(2)} ${s.unit}` : `${ar.toFixed(2)} ${s.unit}² (perímetro ${length.toFixed(2)} ${s.unit})`;
            return { shape: closed ? "polygon" : pts.length > 2 ? "polyline" : "line", length, area: ar, unit: s.unit, text };
          }
          case "measure_save": {
            const s = args.scale as { unit: string; units_per_point: number };
            const pts = args.points as number[][];
            const length = dist(pts) * s.units_per_point;
            return { measurement: { shape: "line", length, area: null, unit: s.unit, text: `${length.toFixed(2)} ${s.unit}` }, undo_depth: 1 };
          }
          case "plugin:event|listen": return nextId++;
          default: return cmd.startsWith("plugin:") ? null : null;
        }
      },
    };
  }, DOC);
}

const calls = (page: Page, cmd: string) =>
  page.evaluate((c) => ((window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls).filter((x) => x.cmd === c), cmd);

async function openTool(page: Page, query: string, option: RegExp) {
  await page.keyboard.press("Control+k");
  await page.getByRole("combobox", { name: "Paleta de órdenes" }).fill(query);
  await page.getByRole("option", { name: option }).first().click();
}

test.describe("Medir y Presentación (backend simulado)", () => {
  test.use({
    locale: "es-ES",
    userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36",
  });

  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "claro", "una sola variante basta");
    await installBackend(page);
    await page.goto("/");
    await page.waitForSelector(".pl-window", { timeout: 45_000 });
    await expect(page.locator(".page").first()).toBeVisible();
  });

  test("Medir: calibrar, medir una distancia y un área con la geometría del zoom", async ({ page }, info) => {
    await openTool(page, "medir", /^Medir/);
    const layer = page.getByTestId("measure-layer").first();
    await expect(layer).toBeVisible();
    const box = (await layer.boundingBox())!;

    // Calibrar: dos clics a 200 px de distancia en pantalla = 10 m reales.
    await page.getByRole("radio", { name: "Calibrar" }).click();
    await page.mouse.click(box.x + 40, box.y + 60);
    await page.mouse.click(box.x + 240, box.y + 60);
    await page.getByLabel("Medida real").fill("10");
    await page.getByRole("button", { name: "Aplicar calibración" }).click();
    const cal = (await calls(page, "measure_calibrate"))[0]!.args as { pagePoints: number; real: number; unit: string };
    expect(cal.real).toBe(10);
    expect(cal.unit).toBe("m");
    expect(cal.pagePoints).toBeGreaterThan(50); // puntos del papel = píxeles / zoom
    await expect(page.getByText(/^Escala: 1 pt = /)).toBeVisible();

    // Distancia: 400 px en pantalla deben dar el doble que la calibración (20 m), sea cual sea el zoom.
    await page.getByRole("radio", { name: "Distancia" }).click();
    await page.mouse.click(box.x + 40, box.y + 120);
    await page.mouse.click(box.x + 440, box.y + 120);
    await expect(page.locator(".pl-measure-result")).toContainText("20.00 m");
    await page.screenshot({ path: info.outputPath("medir-distancia.png") });
    await page.getByRole("button", { name: "Guardar medida" }).click();
    const saved = (await calls(page, "measure_save"))[0]!.args;
    expect(saved).toMatchObject({ docId: 1, path: DOC, page: 0, closed: false });
    await expect(page.getByText("Medida guardada en el documento: 20.00 m")).toBeVisible();

    // Área: cuadrado de 200×200 px = (10 m)² = 100 m².
    await page.getByRole("radio", { name: "Área" }).click();
    for (const [x, y] of [[60, 200], [260, 200], [260, 400], [60, 400]] as const) await page.mouse.click(box.x + x, box.y + y);
    await expect(page.locator(".pl-measure-result")).toContainText("100.00 m²");
    await page.screenshot({ path: info.outputPath("medir-area.png") });
  });

  test("Medir: usar una escala 1:100 y mensaje claro si falta la escala", async ({ page }) => {
    await openTool(page, "medir", /^Medir/);
    await page.getByRole("radio", { name: "Calibrar" }).click();
    await page.getByLabel("Escala 1:").fill("100");
    await page.getByRole("button", { name: "Usar esta escala" }).click();
    expect((await calls(page, "measure_ratio"))[0]?.args).toEqual({ n: 100, unit: "m" });
    await page.getByLabel("Medida real").fill("0");
    await page.getByRole("button", { name: "Aplicar calibración" }).isDisabled();
  });

  test("Presentación: F11 oculta el armazón, las flechas cambian de página y Esc vuelve", async ({ page }, info) => {
    const status = page.locator(".pl-status");
    await expect(status).toContainText("1");
    await page.keyboard.press("F11");
    await expect(page.locator("html[data-presenting]")).toHaveCount(1);
    await expect(page.locator(".pl-titlebar")).toBeHidden();
    await expect(page.locator(".pl-modebar")).toBeHidden();
    await page.waitForTimeout(300);
    await page.screenshot({ path: info.outputPath("presentacion-p1.png") });
    // La página cabe entera en el visor (ni recortada arriba ni abajo).
    const fits = async (n: number) => {
      const view = (await page.getByTestId("viewer").boundingBox())!;
      const box = (await page.locator(`.page[data-page="${n}"]`).boundingBox())!;
      return box.y >= view.y - 1 && box.y + box.height <= view.y + view.height + 1;
    };
    await expect.poll(() => fits(0)).toBe(true);
    await page.keyboard.press("ArrowRight");
    await expect.poll(() => fits(1)).toBe(true);
    await page.screenshot({ path: info.outputPath("presentacion-p2.png") });
    await page.keyboard.press("End");
    await expect.poll(() => fits(2)).toBe(true);
    await page.keyboard.press("ArrowRight"); // tope: no pasa de la última
    await expect.poll(() => fits(2)).toBe(true);
    await page.keyboard.press("Home");
    await expect.poll(() => fits(0)).toBe(true);
    await page.keyboard.press("Escape");
    await expect(page.locator("html[data-presenting]")).toHaveCount(0);
    await expect(page.locator(".pl-titlebar")).toBeVisible();
  });
});
