import { createSignal } from "solid-js";
import type { NoticeTone } from "./Display";

export interface ToastOptions { tone?: NoticeTone; text: string; action?: { label: string; onAction: () => void }; /** ms; 0 = hasta que se cierre. Por defecto 5000 (errores: no se cierran solos). */ timeout?: number }
export interface ToastItem extends ToastOptions { id: number }

const [toasts, setToasts] = createSignal<ToastItem[]>([]);
let next = 1;

/** Muestra un aviso breve no bloqueante. Devuelve una función para cerrarlo. */
export function toast(options: ToastOptions): () => void {
  const id = next++;
  // El mismo aviso repetido (p. ej. el mismo error al reintentar) no se apila: sustituye al anterior.
  setToasts((l) => [...l.filter((t) => t.text !== options.text || (t.tone ?? "info") !== (options.tone ?? "info")), { ...options, id }].slice(-4));
  return () => dismiss(id);
}
/** Avisos visibles ahora (pruebas y lectores de estado). */
export const activeToasts = toasts;
export function dismiss(id: number): void { setToasts((l) => l.filter((t) => t.id !== id)); }
export function clearToasts(): void { setToasts([]); }

