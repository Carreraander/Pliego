/** «Hacer accesible»: del formulario a las opciones de `make_accessible` (`pliego_a11y::FixOptions`). Puro, para probarlo. */
import type { A11yReport } from "./report";

/** Idiomas que se ofrecen; si el documento ya tiene otro, también se muestra. */
export const LANGUAGES = ["es-ES", "en-US", "en-GB", "ca-ES", "eu-ES", "gl-ES", "fr-FR", "pt-PT", "pt-BR", "de-DE", "it-IT"];

export interface FixOptions {
  autotag: boolean;
  language: string | null;
  title: string | null;
  alt_texts: [number, string][];
  tab_order: boolean;
}

export interface FixForm {
  tagged: boolean;
  autotag: boolean;
  title: string;
  language: string;
  tabOrder: boolean;
  /** Texto alternativo por número de objeto de la figura. */
  alts: Record<number, string>;
}

export interface Figure { xref: number; page: number | null }

/** Figuras sin texto alternativo que el comprobador encontró (solo en documentos etiquetados). */
export const figuresWithoutAlt = (r: A11yReport): Figure[] =>
  r.issues.filter((i) => i.code === "figure_without_alt" && typeof i.xref === "number").map((i) => ({ xref: i.xref!, page: i.page }));

/** Idioma propuesto: el del documento o el de la interfaz. */
export const defaultLanguage = (r: A11yReport, uiLocale: string): string => r.language ?? (uiLocale.startsWith("en") ? "en-US" : "es-ES");

/** Solo se manda lo que cambia; `null` si no queda nada que hacer. */
export function fixOptions(f: FixForm, r: A11yReport): FixOptions | null {
  const title = f.title.trim();
  const autotag = !f.tagged && f.autotag;
  const opts: FixOptions = {
    autotag,
    title: title && title !== (r.title ?? "") ? title : null,
    language: f.language && f.language !== r.language ? f.language : null,
    alt_texts: Object.entries(f.alts).filter(([, v]) => v.trim()).map(([k, v]) => [Number(k), v.trim()]),
    tab_order: f.tabOrder && (f.tagged || autotag),
  };
  return opts.autotag || opts.title || opts.language || opts.alt_texts.length || opts.tab_order ? opts : null;
}
