//! Lógica pura del panel (sin interfaz): citas clicables, tamaños y conversación por documento.
import { createSignal } from "solid-js";
import type { AssistantProgress, Citation } from "./api";

export type Segment = { kind: "text"; text: string } | { kind: "page"; page: number };

const CITE = /\[\s*(?:p\.|pp\.|pág\.|págs\.|pag\.|pags\.|page|pages)\s*([0-9][0-9\s,;\-–y]*)\]/gi;

/** Parte la respuesta en texto y citas `[p. 3]`; cada página citada (desde 0) se convierte en un botón. */
export function splitCitations(text: string): Segment[] {
  const out: Segment[] = [];
  let last = 0;
  for (const m of text.matchAll(CITE)) {
    if (m.index > last) out.push({ kind: "text", text: text.slice(last, m.index) });
    for (const n of (m[1] ?? "").matchAll(/([0-9]+)(?:\s*[-–]\s*([0-9]+))?/g)) {
      const a = Number(n[1]);
      const b = n[2] ? Number(n[2]) : a;
      if (a > 0 && b >= a && b - a < 50) for (let p = a; p <= b; p++) out.push({ kind: "page", page: p - 1 });
    }
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ kind: "text", text: text.slice(last) });
  return out;
}

/** Páginas (desde 0) que respaldan la respuesta: las del modelo y, si no hay, la mejor fuente recuperada. */
export function sourcePages(citations: Citation[]): number[] {
  return [...new Set(citations.map((c) => c.page))].sort((a, b) => a - b);
}

/** «1,1 GB» / «820 MB» a partir de megabytes. */
export function formatSize(mb: number, lang = "es"): string {
  if (mb >= 1000) return `${new Intl.NumberFormat(lang, { maximumFractionDigits: 1 }).format(mb / 1000)} GB`;
  return `${Math.round(mb)} MB`;
}

/** Fracción 0..1 del progreso, o `null` si no se conoce el total. */
export function progressFraction(p: Pick<AssistantProgress, "done" | "total"> | null): number | null {
  if (!p || p.total <= 0) return null;
  return Math.min(1, p.done / p.total);
}

export function newJobId(): string {
  return `assistant-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

// ── Conversación por documento (en memoria: no se guarda nada en disco) ─────────────────
export type ChatMessage =
  | { id: number; role: "user"; text: string }
  | { id: number; role: "assistant"; text: string; citations: Citation[] }
  | { id: number; role: "error"; text: string };

const [chats, setChats] = createSignal<Record<string, ChatMessage[]>>({});
let seq = 0;

export const chatOf = (docId: string): ChatMessage[] => chats()[docId] ?? [];

export function addMessage(docId: string, m: DistributiveOmit<ChatMessage, "id">): void {
  setChats((all) => ({ ...all, [docId]: [...(all[docId] ?? []), { ...m, id: ++seq } as ChatMessage] }));
}
export function clearChat(docId: string): void {
  setChats((all) => ({ ...all, [docId]: [] }));
}
type DistributiveOmit<T, K extends keyof never> = T extends unknown ? Omit<T, K> : never;
