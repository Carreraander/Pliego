/** Datos de «Propiedades del documento» (W29): arma las filas a partir de la información del motor y del archivo. */
export interface DocFacts {
  page_count: number;
  title: string | null; author: string | null;
  subject?: string | null; keywords?: string | null; creator?: string | null; producer?: string | null;
  format?: string | null; encryption?: string | null;
}
export interface FileFacts { size: number; modified_secs: number | null }
export interface PropertyRow { key: string; value: string }

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1).replace(".", ",")} KB`;
  return `${(kb / 1024).toFixed(1).replace(".", ",")} MB`;
}

/** «210 × 297 mm» a partir de puntos PDF (72 por pulgada). */
export function pageSizeLabel(width: number, height: number): string {
  const mm = (pt: number) => Math.round((pt * 25.4) / 72);
  return `${mm(width)} × ${mm(height)} mm`;
}

/** Filas con valor, en el orden en que se muestran. Los campos vacíos se omiten. */
export function propertyRows(
  info: DocFacts, file: FileFacts | null, path: string, firstPage: { width: number; height: number } | null,
  date: (secs: number) => string,
): PropertyRow[] {
  const rows: (PropertyRow | null)[] = [
    row("title", info.title), row("author", info.author), row("subject", info.subject), row("keywords", info.keywords),
    row("creator", info.creator), row("producer", info.producer), row("format", info.format),
    { key: "pages", value: String(info.page_count) },
    firstPage ? { key: "pageSize", value: pageSizeLabel(firstPage.width, firstPage.height) } : null,
    file ? { key: "fileSize", value: formatSize(file.size) } : null,
    file?.modified_secs != null ? { key: "modified", value: date(file.modified_secs) } : null,
    row("encryption", info.encryption),
    row("path", path),
  ];
  return rows.filter((r): r is PropertyRow => r !== null);
}

function row(key: string, value: string | null | undefined): PropertyRow | null {
  const v = value?.trim();
  return v ? { key, value: v } : null;
}

/** Las propiedades que se pueden cambiar desde el panel. */
export const EDITABLE = ["title", "author", "subject", "keywords"] as const;
export type EditableKey = (typeof EDITABLE)[number];
export type InfoForm = Record<EditableKey, string>;

export const infoForm = (info: DocFacts): InfoForm => ({
  title: info.title?.trim() ?? "", author: info.author?.trim() ?? "", subject: info.subject?.trim() ?? "", keywords: info.keywords?.trim() ?? "",
});

/** Lo que cambió respecto al documento (`pliego_pdfa::InfoEdit`; texto vacío = quitar), o `null` si nada. */
export function infoEdit(before: InfoForm, now: InfoForm): Partial<InfoForm> | null {
  const edit: Partial<InfoForm> = {};
  for (const k of EDITABLE) if (now[k].trim() !== before[k]) edit[k] = now[k].trim();
  return Object.keys(edit).length ? edit : null;
}
