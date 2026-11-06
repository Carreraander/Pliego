import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { orderedRulePairs } from "../../jobs/translate";
import { CLI_HELP_RULES } from "./rules-cli-help";

/**
 * La CLI traduce sus mensajes con las mismas reglas que la interfaz (`rules.json`) y la ayuda de `--help` con las suyas
 * (`help.json`, coincidencia exacta), leídas desde JSON generados.
 * Esta prueba lo mantiene al día: si cambian las reglas, ejecuta `UPDATE_CLI_RULES=1 npx vitest run cli-rules` y sube el JSON.
 */
const dir = join(__dirname, "../../../../crates/pliego-cli/src/i18n");

const lines = (pairs: readonly (readonly [string, string])[]): string => `[\n${pairs.map((p) => `  ${JSON.stringify(p)}`).join(",\n")}\n]\n`;

describe("reglas de la CLI", () => {
  for (const [name, json] of [["rules.json", lines(orderedRulePairs())], ["help.json", lines(CLI_HELP_RULES)]] as const) {
    it(`${name}, el que lee la CLI, está al día`, () => {
      if (process.env.UPDATE_CLI_RULES) writeFileSync(join(dir, name), json);
      expect(readFileSync(join(dir, name), "utf8")).toBe(json);
    });
  }
});
