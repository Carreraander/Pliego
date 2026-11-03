import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

// Puerto fijo: lo espera tauri.conf.json (devUrl).
export default defineConfig({
  plugins: [solid()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", outDir: "dist" },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
