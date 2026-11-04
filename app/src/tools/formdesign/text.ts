import { locale } from "../../i18n";
import { toolText } from "../strings";

/** Textos del diseñador de formularios (se registran en `strings.ts`) con sustitución de `{n}`. */
export function ft(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
