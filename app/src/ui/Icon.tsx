import { Dynamic } from "solid-js/web";
import { splitProps, type Component, type JSX } from "solid-js";
import {
  AlertTriangle, ArrowLeft, ArrowRight, Check, CheckCircle2, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, Copy,
  FileDown, FilePlus, FileText, FolderOpen, Hand, Highlighter, Info, Layers, Lock, MessageSquare, Minus, MousePointer2,
  PanelLeft, PanelRight, Paperclip, Pencil, Plus, RotateCcw, RotateCw, Search, Settings, ShieldCheck, Square, Star,
  Stamp, Trash2, Type, Underline, X, XCircle, Bookmark, LayoutGrid, Download, ScanText, Scissors, Signature, Wrench,
  RefreshCw, Maximize2, Minimize2, MoreHorizontal, Undo2, Redo2, ZoomIn, ZoomOut,
} from "lucide-solid";
import { BatesIcon, OcrIcon, PdfAIcon, RedactIcon, SignDigitalIcon, TimestampIcon, type IconProps } from "./icons/custom";

type IconComponent = Component<IconProps>;

/** Registro de iconos por nombre (los nombres de `ToolManifest.icon` salen de aquí). Los flujos pueden añadir más. */
const registry: Record<string, IconComponent> = {
  "alert-triangle": AlertTriangle, "arrow-left": ArrowLeft, "arrow-right": ArrowRight, bookmark: Bookmark, check: Check,
  "check-circle": CheckCircle2, "chevron-down": ChevronDown, "chevron-left": ChevronLeft, "chevron-right": ChevronRight,
  "chevron-up": ChevronUp, copy: Copy, download: Download, "file-down": FileDown, "file-plus": FilePlus,
  "file-text": FileText, "folder-open": FolderOpen, hand: Hand, highlighter: Highlighter, info: Info, layers: Layers,
  "layout-grid": LayoutGrid, lock: Lock, "message-square": MessageSquare, minus: Minus, "mouse-pointer": MousePointer2,
  "panel-left": PanelLeft, "panel-right": PanelRight, paperclip: Paperclip, pencil: Pencil, plus: Plus, "rotate-ccw": RotateCcw,
  "rotate-cw": RotateCw, search: Search, settings: Settings, "shield-check": ShieldCheck, square: Square, star: Star,
  stamp: Stamp, "trash": Trash2, type: Type, underline: Underline, x: X, "x-circle": XCircle, "scan-text": ScanText,
  scissors: Scissors, signature: Signature, wrench: Wrench, refresh: RefreshCw, maximize: Maximize2, minimize: Minimize2,
  more: MoreHorizontal, undo: Undo2, redo: Redo2, "zoom-in": ZoomIn, "zoom-out": ZoomOut,
  // Propios de Pliego
  redact: RedactIcon, bates: BatesIcon, "pdf-a": PdfAIcon, ocr: OcrIcon, timestamp: TimestampIcon, "sign-digital": SignDigitalIcon,
};

export type IconName = string;

/** Permite a un flujo registrar iconos propios (`registerIcon("mi-icono", MiIcono)`). */
export function registerIcon(name: string, component: IconComponent): void {
  registry[name] = component;
}

export function hasIcon(name: string): boolean {
  return name in registry;
}

/** Icono decorativo por nombre. Un icono nunca va solo sin nombre accesible: ponlo dentro de un `IconButton`. */
export function Icon(props: { name: IconName; size?: number } & JSX.SvgSVGAttributes<SVGSVGElement>) {
  const [own, rest] = splitProps(props, ["name", "size"]);
  return <Dynamic component={registry[own.name] ?? registry["info"]} size={own.size ?? 20} strokeWidth={1.75} {...rest} />;
}
