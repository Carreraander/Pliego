import { locale } from "../../i18n";
import { toolText } from "../strings";

/** Textos de las herramientas de Comentar (se registran en `strings.ts`) con sustitución de `{n}`. */
export function at(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
