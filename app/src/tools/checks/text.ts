import { locale } from "../../i18n";
import { toolText } from "../strings";

/** Textos de las comprobaciones (se registran en `strings.ts`) con sustitución de `{n}`. */
export function ct(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
