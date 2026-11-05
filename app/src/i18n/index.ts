import { createSignal } from "solid-js";
import en from "./en.json";
import es from "./es.json";

export type Locale = "es" | "en";
export type Key = keyof typeof es;

const dictionaries: Record<Locale, Record<string, string>> = { es, en };

/** Elige el idioma según el del sistema; español por defecto. */
export function detectLocale(lang: string | undefined): Locale {
  return lang?.toLowerCase().startsWith("en") ? "en" : "es";
}

// Español hasta que se cargan los ajustes (el idioma por defecto del producto); «Automático» lo cambia luego.
const [locale, setLocale] = createSignal<Locale>("es");
export { locale, setLocale };

/** Traduce una clave y sustituye `{nombre}` con los valores dados. Si falta la clave en el idioma, usa español. */
export function translate(loc: Locale, key: Key, vars: Record<string, string | number> = {}): string {
  const text = dictionaries[loc][key] ?? es[key] ?? key;
  return text.replace(/\{(\w+)\}/g, (_, name: string) => String(vars[name] ?? `{${name}}`));
}

export function t(key: Key, vars?: Record<string, string | number>): string {
  return translate(locale(), key, vars);
}
