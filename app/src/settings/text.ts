import { locale, type Locale } from "../i18n";

/** Diccionario por idioma con sustitución de `{nombre}`; el español es el de reserva. */
export function scoped<K extends string>(dict: Record<Locale, Record<K, string>>): (key: K, vars?: Record<string, string | number>) => string {
  return (key, vars = {}) => {
    const text: string = dict[locale()][key] ?? dict.es[key];
    return text.replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
  };
}
