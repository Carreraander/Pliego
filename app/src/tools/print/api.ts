/** Tipos y comandos de Imprimir (W45). Los nombres vienen en camelCase de `pliego-printer`. */
import { invoke } from "@tauri-apps/api/core";

export type Orientation = "auto" | "portrait" | "landscape";
export type ScaleMode = "fit" | "shrink" | "actual" | "custom";
export type Duplex = "off" | "longEdge" | "shortEdge";
export type Subset = "all" | "odd" | "even";

export interface PrintOptions {
  printer: string | null;
  pages: string;
  subset: Subset;
  reverse: boolean;
  copies: number;
  collate: boolean;
  orientation: Orientation;
  autoRotate: boolean;
  scale: ScaleMode;
  scalePercent: number;
  duplex: Duplex;
  grayscale: boolean;
  comments: boolean;
  paper: number | null;
  output: string | null;
}

export interface PrinterEntry { name: string; isDefault: boolean }
export interface Paper { id: number; name: string; widthMm: number; heightMm: number }
export interface PrinterCaps {
  name: string;
  color: boolean;
  duplex: boolean;
  maxCopies: number;
  collate: boolean;
  papers: Paper[];
  defaultPaper: number | null;
}
/** Hoja en puntos (`dpi` 72): papel y zona imprimible. */
export interface SheetMetrics {
  dpiX: number; dpiY: number;
  physW: number; physH: number;
  offX: number; offY: number;
  areaW: number; areaH: number;
}
/** Dónde cae la página en la zona imprimible (mismas unidades que `SheetMetrics`). */
export interface Placement { rotate: number; x: number; y: number; w: number; h: number }

export const printApi = {
  printers: () => invoke<PrinterEntry[]>("print_printers"),
  caps: (name: string) => invoke<PrinterCaps>("print_printer_caps", { name }),
  sheet: (name: string, paper: number | null, landscape: boolean) =>
    invoke<SheetMetrics>("print_sheet", { name, paper, landscape }),
  layout: (pageWidth: number, pageHeight: number, sheet: SheetMetrics, options: PrintOptions) =>
    invoke<Placement>("print_preview_layout", { pageWidth, pageHeight, sheet, options }),
};
