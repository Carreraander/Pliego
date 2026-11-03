import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";

// Chromium preinstalado en los entornos de la nube; en otros sitios, el que descargue Playwright.
const preinstalled = "/opt/pw-browsers/chromium";

export default defineConfig({
  testDir: "tests/visual",
  snapshotPathTemplate: "{testDir}/__screenshots__/{projectName}/{arg}{ext}",
  fullyParallel: true,
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.01, animations: "disabled" } },
  use: {
    locale: "es-ES",
    baseURL: "http://127.0.0.1:1420",
    launchOptions: existsSync(preinstalled) ? { executablePath: preinstalled, args: ["--no-sandbox"] } : {},
  },
  projects: [
    { name: "claro", use: { colorScheme: "light", viewport: { width: 1280, height: 900 } } },
    { name: "oscuro", use: { colorScheme: "dark", viewport: { width: 1280, height: 900 } } },
    { name: "alto-contraste", use: { colorScheme: "light", forcedColors: "active", viewport: { width: 1280, height: 900 } } },
  ],
  webServer: { command: "npm run dev", url: "http://127.0.0.1:1420/ui-catalog", reuseExistingServer: true, timeout: 60_000 },
});
