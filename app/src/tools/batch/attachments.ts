/** Resultado de `portfolio_list` (`pliego_portfolio::Listing`). */
export interface PortfolioEntry { name: string; filename: string; mime_type: string | null; size: number }
export interface PortfolioListing { is_portfolio: boolean; entries: PortfolioEntry[] }

/** Tamaño legible: «1,2 MB». */
export function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1).replace(".", ",")} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1).replace(".", ",")} MB`;
}
