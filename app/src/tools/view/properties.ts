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
