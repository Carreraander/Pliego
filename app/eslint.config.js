import js from "@eslint/js";
import globals from "globals";
import solid from "eslint-plugin-solid/configs/typescript";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "node_modules", "src-tauri", "service", "test-results", "tests/visual/*-snapshots"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  { files: ["src/**/*.{ts,tsx}"], ...solid, languageOptions: { ...solid.languageOptions, globals: globals.browser } },
  { rules: { "@typescript-eslint/no-unused-expressions": ["error", { allowShortCircuit: true, allowTernary: true }] } },
  { files: ["*.config.{js,ts}", "tests/**/*.ts", "scripts/**/*.{js,mjs}"], languageOptions: { globals: globals.node } },
);
