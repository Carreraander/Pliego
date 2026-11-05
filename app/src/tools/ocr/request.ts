/** Argumentos de los comandos de OCR (puro, con pruebas). */

export interface OcrLanguages { installed: string[]; downloadable: string[] }

export const OCR_DPI = [300, 400, 600] as const;

export interface OcrForm {
  path: string;
  langs: string[];
  pages: string;
  force: boolean;
  deskew: boolean;
  rotate: boolean;
  binarize: boolean;
  denoise: boolean;
  dpi: number;
  suffix: string;
}

/** Valida un rango de páginas («all», «1-3,5»); vacío = todas. Devuelve el texto normalizado o `null`. */
export function normalizePages(text: string): string | null {
  const t = text.trim().toLowerCase();
  if (t === "" || t === "all" || t === "todas") return "all";
  return /^\d+(-\d+)?(\s*,\s*\d+(-\d+)?)*$/.test(t) ? t.replace(/\s+/g, "") : null;
}

/** Idiomas elegidos por defecto: español e inglés si están instalados; si no, el primero que haya. */
export function defaultLanguages(installed: string[]): string[] {
  const wanted = ["spa", "eng"].filter((l) => installed.includes(l));
  return wanted.length ? wanted : installed.slice(0, 1);
}

/** Petición del comando `ocr_pdf`; `null` si el formulario no es válido (sin idiomas o páginas mal escritas). */
export function ocrArgs(f: OcrForm): { request: Record<string, unknown> } | null {
  const pages = normalizePages(f.pages);
  if (!f.path || f.langs.length === 0 || pages === null) return null;
  return {
    request: {
      path: f.path, langs: f.langs.join("+"), pages, force: f.force, deskew: f.deskew, rotate: f.rotate,
      binarize: f.binarize, denoise: f.denoise, dpi: f.dpi, suffix: f.suffix,
    },
  };
}

/** Nombre legible de un código de Tesseract; los desconocidos se muestran tal cual. */
const NAMES: Record<string, [string, string]> = {
  spa: ["Español", "Spanish"], eng: ["Inglés", "English"], fra: ["Francés", "French"], deu: ["Alemán", "German"],
  ita: ["Italiano", "Italian"], por: ["Portugués", "Portuguese"], cat: ["Catalán", "Catalan"], eus: ["Euskera", "Basque"],
  glg: ["Gallego", "Galician"], nld: ["Neerlandés", "Dutch"], osd: ["Orientación y escritura", "Orientation and script"],
};
export function languageName(code: string, es: boolean): string {
  const n = NAMES[code];
  return n ? `${es ? n[0] : n[1]} (${code})` : code;
}
