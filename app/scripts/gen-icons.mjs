// Genera los iconos de la app a partir del logo (docs/design/sistema-visual.md §Logo):
//   node scripts/gen-icons.mjs [carpeta de salida, por defecto src-tauri/icons]
// Produce icon.ico (16–256 px, PNG incrustados), 32x32.png, 128x128.png, 128x128@2x.png, icon.png (512) y logo.svg.
// Renderiza con el Chromium de Playwright (ya es dependencia de desarrollo): sin dependencias nuevas.
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const here = dirname(fileURLToPath(import.meta.url));
const out = resolve(process.argv[2] ?? join(here, "../src-tauri/icons"));
mkdirSync(out, { recursive: true });

// Colores de tokens.css (--pl-accent y --pl-accent-soft del tema claro); aquí hacen falta literales porque el SVG sale de la app.
const SHEET = "#0f6b62";
const FOLD = "#d7ebe7";
const svg = (size) => `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 24 24">
  <path d="M5 2h10l7 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z" fill="${SHEET}"/>
  <path d="M15 2v5a2 2 0 0 0 2 2h5z" fill="${FOLD}"/>
  <path d="M15 2l7 7" stroke="${SHEET}" stroke-width="0.8" fill="none"/>
</svg>`;

const executablePath = existsSync("/opt/pw-browsers/chromium") ? "/opt/pw-browsers/chromium" : undefined;
const browser = await chromium.launch(executablePath ? { executablePath, args: ["--no-sandbox"] } : {});
const page = await browser.newPage({ deviceScaleFactor: 1 });

async function png(size) {
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(`<body style="margin:0;background:transparent">${svg(size)}</body>`);
  return page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } });
}

const sizes = [16, 20, 24, 32, 48, 64, 128, 256, 512];
const images = {};
for (const s of sizes) images[s] = await png(s); // en serie: comparten la misma página
await browser.close();

// .ico con PNG incrustados (válido desde Windows Vista)
const icoSizes = [16, 20, 24, 32, 48, 64, 256];
const header = Buffer.alloc(6); header.writeUInt16LE(1, 2); header.writeUInt16LE(icoSizes.length, 4);
let offset = 6 + 16 * icoSizes.length;
const dir = []; const data = [];
for (const s of icoSizes) {
  const buf = images[s]; const e = Buffer.alloc(16);
  e.writeUInt8(s === 256 ? 0 : s, 0); e.writeUInt8(s === 256 ? 0 : s, 1); e.writeUInt16LE(1, 4); e.writeUInt16LE(32, 6);
  e.writeUInt32LE(buf.length, 8); e.writeUInt32LE(offset, 12);
  offset += buf.length; dir.push(e); data.push(buf);
}
writeFileSync(join(out, "icon.ico"), Buffer.concat([header, ...dir, ...data]));
writeFileSync(join(out, "32x32.png"), images[32]);
writeFileSync(join(out, "128x128.png"), images[128]);
writeFileSync(join(out, "128x128@2x.png"), images[256]);
writeFileSync(join(out, "icon.png"), images[512]);
writeFileSync(join(out, "logo.svg"), svg(24).replace('width="24" height="24" ', ""));
console.log(`Iconos escritos en ${out}`);
