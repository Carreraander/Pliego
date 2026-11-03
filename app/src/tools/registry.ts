/**
 * Registro de herramientas (docs/design/ux-y-navegacion.md §10). La barra de modos, la paleta «Herramientas», la paleta
 * de órdenes, los atajos y el menú contextual se generan a partir de él: un flujo añade su herramienta con un
 * `ToolManifest` y una línea de registro, sin tocar el armazón.
 */
import { createSignal, type Component } from "solid-js";
import { locale } from "../i18n";
import { toolText } from "./strings";

export type ToolKind = "mode" | "task" | "wizard" | "panel" | "screen";

/** Grupos de la paleta «Herramientas» (§4.3), en orden de aparición. */
export const TOOL_GROUPS = [
  "view", "comment", "fill-sign", "organize", "edit", "forms", "scan-ocr", "protect", "convert", "compare-optimize",
  "standards", "print", "automate", "assistant",
] as const;
export type ToolGroup = (typeof TOOL_GROUPS)[number];

/** Requisitos de la herramienta; si alguno no se cumple queda desactivada con el motivo. */
export interface ToolCondition {
  /** Necesita un documento abierto. */
  document?: boolean;
  /** El documento tiene campos de formulario. */
  forms?: boolean;
  /** El documento tiene firmas. */
  signatures?: boolean;
  /** El documento está protegido (cifrado). `false` = solo si no lo está. */
  protected?: boolean;
  /** El documento es PDF/A. */
  pdfa?: boolean;
}

/** Hechos del documento activo que evalúan las condiciones `when`. */
export interface DocFacts {
  document: boolean;
  forms: boolean;
  signatures: boolean;
  protected: boolean;
  pdfa: boolean;
}
export const NO_DOCUMENT: DocFacts = { document: false, forms: false, signatures: false, protected: false, pdfa: false };

export interface ToolManifest {
  /** Único, `<area>.<herramienta>`. */
  id: string;
  group: ToolGroup;
  kind: ToolKind;
  /** Claves del diccionario de herramientas (`tools/strings.ts`) o, con `literal`, texto ya traducido por idioma. */
  title: string;
  description: string;
  /** Nombre de icono (`app/src/ui/Icon`). */
  icon: string;
  keywords?: string[];
  /** Atajo opcional, p. ej. `Ctrl+Alt+C`. En modos de una sola letra solo vale con el foco en el lienzo. */
  shortcut?: string;
  when?: ToolCondition;
  pinnable?: boolean;
  /** Componente perezoso: panel de tarea (`task`/`wizard`), contenido de `screen`… */
  load?: () => Promise<{ default: Component<any> }>; // eslint-disable-line @typescript-eslint/no-explicit-any
  /** `mode`: id del modo registrado con `registerMode` que se activa. */
  modeId?: string;
  /** `panel`: id del panel del raíl (`registerPanel`) que se abre. */
  panelId?: string;
  /** Aún no existe: se muestra desactivada con «Disponible más adelante» (los flujos la sustituyen al registrar la real). */
  ghost?: boolean;
}

const [manifests, setManifests] = createSignal<ToolManifest[]>([]);

/** Registra (o sustituye, por id) una herramienta. Devuelve la función para retirarla. */
export function registerTool(manifest: ToolManifest): () => void {
  setManifests((all) => [...all.filter((m) => m.id !== manifest.id), manifest]);
  return () => setManifests((all) => all.filter((m) => m !== manifest));
}
export const tools = manifests;
export const getTool = (id: string): ToolManifest | undefined => manifests().find((m) => m.id === id);

/** Herramientas de un grupo en el orden en que se registraron (las reales antes que las fantasma). */
export function toolsByGroup(): { group: ToolGroup; tools: ToolManifest[] }[] {
  return TOOL_GROUPS.map((group) => ({
    group,
    tools: manifests().filter((m) => m.group === group).sort((a, b) => Number(!!a.ghost) - Number(!!b.ghost)),
  })).filter((g) => g.tools.length > 0);
}

// ── Estado del documento ─────────────────────────────────────────────────────────────
const [facts, setFacts] = createSignal<DocFacts>(NO_DOCUMENT);
export const docFacts = facts;
/** Lo llama quien conoce el documento activo (W01 y, para formularios/firmas, sus flujos). */
export function setDocFacts(next: Partial<DocFacts>): void { setFacts((f) => ({ ...f, ...next })); }

export type ToolState = { enabled: true } | { enabled: false; reason: string };

/** Evalúa `when` y `ghost`; el motivo sale en el idioma activo. */
export function toolState(tool: ToolManifest, f: DocFacts = facts()): ToolState {
  const loc = locale();
  if (tool.ghost) return { enabled: false, reason: toolText("reason.soon", loc) };
  const w = tool.when;
  if (!w) return { enabled: true };
  if (w.document && !f.document) return { enabled: false, reason: toolText("reason.document", loc) };
  if (w.forms && !f.forms) return { enabled: false, reason: toolText("reason.forms", loc) };
  if (w.signatures && !f.signatures) return { enabled: false, reason: toolText("reason.signatures", loc) };
  if (w.protected !== undefined && f.protected !== w.protected) {
    return { enabled: false, reason: toolText(w.protected ? "reason.protected" : "reason.notProtected", loc) };
  }
  if (w.pdfa && !f.pdfa) return { enabled: false, reason: toolText("reason.pdfa", loc) };
  return { enabled: true };
}

export const toolTitle = (tool: ToolManifest): string => toolText(tool.title, locale());
export const toolDescription = (tool: ToolManifest): string => toolText(tool.description, locale());
