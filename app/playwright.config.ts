import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";

// Chromium preinstalado en los entornos de la nube; en otros sitios, el que descargue Playwright.
const preinstalled = "/opt/pw-browsers/chromium";

export default defineConfig({
  testDir: "tests/visual",
  snapshotPathTemplate: "{testDir}/__screenshots__/{projectName}/{arg}{ext}",
  // Vite en frío tarda en servir la primera página (visto en acr12): margen generoso.
  timeout: 60_000,
  fullyParallel: true,
  // Las capturas de referencia dependen de las fuentes del sistema: en CI solo se comprueban teclado y accesibilidad.
  ignoreSnapshots: !!process.env.CI,
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.01, animations: "disabled" } },
  use: {
    locale: "es-ES",
    actionTimeout: 15_000,
    navigationTimeout: 45_000,
    baseURL: "http://127.0.0.1:1420",
    launchOptions: existsSync(preinstalled) ? { executablePath: preinstalled, args: ["--no-sandbox"] } : {},
  },
  projects: [
    { name: "claro", use: { colorScheme: "light", viewport: { width: 1280, height: 900 } } },
    { name: "oscuro", use: { colorScheme: "dark", viewport: { width: 1280, height: 900 } } },
    { name: "alto-contraste", use: { colorScheme: "light", forcedColors: "active", viewport: { width: 1280, height: 900 } } },
  ],
  webServer: { command: "npm run dev -- --host 127.0.0.1", url: "http://127.0.0.1:1420/ui-catalog", reuseExistingServer: true, timeout: 60_000 },
});
