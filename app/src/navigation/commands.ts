/**
 * Órdenes de la paleta `Ctrl+K` (§4.4). Cada fuente es un proveedor enchufable: herramientas y órdenes, documentos abiertos,
 * recientes, «ir a la página N», buscar texto y ajustes. Los flujos añaden órdenes con `registerCommand` o un proveedor propio.
 */
import { createSignal } from "solid-js";
import { locale } from "../i18n";
import { activeDoc, docs, setActiveId } from "../state/documents";
import { goToPage } from "../state/navigation";
import { runSearch } from "../state/search";
import { docFacts, toolDescription, toolState, toolTitle, tools } from "../tools";
import { activateTool } from "./activate";
import { bestScore, normalize } from "./fuzzy";
import { displayCombo } from "./keys";
import { effectiveKeys, shortcutDefs } from "./shortcuts";
import { nt, type NavKey } from "./strings";

export interface Command {
  id: string;
  title: string;
  subtitle?: string;
  icon?: string;
  /** Atajo para mostrar a la derecha (ya con el formato del idioma). */
  shortcut?: string;
  /** Si está, el resultado se muestra atenuado con este motivo. */
  disabledReason?: string;
  run: () => void;
  /** `Tab`: amplía (abre las opciones de la tarea en el inspector). Sin esto, `Tab` completa el texto. */
  expand?: () => void;
}

export interface CommandProvider {
  id: string;
  /** Orden de los grupos de resultados (menor = antes). */
  order: number;
  /** Clave del título del grupo. */
  group: NavKey;
  query: (q: string) => Command[];
}

const [providers, setProviders] = createSignal<CommandProvider[]>([]);
export function registerCommandProvider(p: CommandProvider): () => void {
  setProviders((all) => [...all.filter((x) => x.id !== p.id), p]);
  return () => setProviders((all) => all.filter((x) => x !== p));
}

export interface ResultGroup { id: string; label: string; items: Command[] }

/** Resultados agrupados por proveedor, con un máximo por grupo. */
export function searchCommands(q: string, perGroup = 8): ResultGroup[] {
  return [...providers()].sort((a, b) => a.order - b.order)
    .map((p) => ({ id: p.id, label: nt(p.group), items: p.query(q).slice(0, perGroup) }))
    .filter((g) => g.items.length > 0);
}

// ── Órdenes sueltas ───────────────────────────────────────────────────────────────────
export interface PlainCommand extends Command { keywords?: string[] }
const [plain, setPlain] = createSignal<PlainCommand[]>([]);
export function registerCommand(c: PlainCommand): () => void {
  setPlain((all) => [...all.filter((x) => x.id !== c.id), c]);
  return () => setPlain((all) => all.filter((x) => x !== c));
}

function ranked<T>(items: T[], q: string, texts: (t: T) => string[], exact: (t: T) => string[] = () => []): T[] {
  if (!q.trim()) return items;
  return items.map((it) => ({ it, s: bestScore(q, texts(it), exact(it)) })).filter((x): x is { it: T; s: number } => x.s !== null)
    .sort((a, b) => b.s - a.s).map((x) => x.it);
}

const shortcutOf = (id: string): string | undefined => {
  const d = shortcutDefs().find((s) => s.id === id);
  const keys = d ? effectiveKeys(d) : "";
  return keys ? displayCombo(keys, locale()) : undefined;
};

const toolsProvider: CommandProvider = {
  id: "nav.tools", order: 10, group: "nav.group.commands",
  query(q) {
    const facts = docFacts();
    const fromTools: PlainCommand[] = tools().map((tool) => {
      const state = toolState(tool, facts);
      return {
        id: `tool:${tool.id}`, title: toolTitle(tool), subtitle: toolDescription(tool), icon: tool.icon,
        shortcut: tool.shortcut ? displayCombo(tool.shortcut.replace(/\bMayús\b/, "Shift"), locale()) : undefined,
        disabledReason: state.enabled ? undefined : state.reason, keywords: tool.keywords,
        run: () => void activateTool(tool),
        expand: tool.kind === "task" || tool.kind === "wizard" ? () => void activateTool(tool) : undefined,
      };
    });
    return ranked([...fromTools, ...plain()], q, (c) => [c.title, ...(c.keywords ?? [])], (c) => (c.subtitle ? [c.subtitle] : []));
  },
};

const documentsProvider: CommandProvider = {
  id: "nav.documents", order: 20, group: "nav.group.documents",
  query: (q) => ranked(docs(), q, (d) => [d.title, d.path]).map((d) => ({
    id: `doc:${d.id}`, title: d.title, subtitle: d.path, icon: "file-text", run: () => setActiveId(d.id),
  })),
};

const gotoProvider: CommandProvider = {
  id: "nav.goto", order: 5, group: "nav.group.pages",
  query(q) {
    const doc = activeDoc();
    const m = /^(?:p(?:ág(?:ina)?|age)?\.?\s*|ir a\s+|go to\s+)?(\d{1,5})$/i.exec(normalize(q).trim().replace(/^pag/, "p"));
    if (!doc || !m) return [];
    const n = Math.min(Math.max(1, Number(m[1])), doc.sizes.length);
    return [{ id: `goto:${n}`, title: nt("nav.goto", { n }), subtitle: nt("nav.gotoOf", { n, total: doc.sizes.length }), icon: "arrow-right", run: () => goToPage(n - 1) }];
  },
};

const textProvider: CommandProvider = {
  id: "nav.text", order: 30, group: "nav.group.text",
  query(q) {
    const doc = activeDoc();
    const text = q.trim();
    if (!doc || text.length < 2 || /^\d+$/.test(text)) return [];
    return [{ id: "search-text", title: nt("nav.searchText", { q: text }), icon: "search", shortcut: displayCombo("Ctrl+F", locale()), run: () => void runSearch(doc.id, text) }];
  },
};

/** Registra las fuentes de base y las órdenes propias de la navegación. Devuelve cómo retirarlas. */
export function installCommandProviders(): () => void {
  const offs = [toolsProvider, documentsProvider, gotoProvider, textProvider].map(registerCommandProvider);
  return () => offs.forEach((off) => off());
}
export { shortcutOf };
