import { createSignal } from "solid-js";

/**
 * Documentos recuperables tras un cierre inesperado. `state/session.ts` llama a `setRecoverable` al arrancar si la
 * ejecución anterior no se cerró bien; Inicio muestra el aviso con [Reabrir] [Descartar] [Copiar informe].
 */
export interface Recoverable { id: string; title: string }
export interface RecoveryHandlers {
  restore: (docs: Recoverable[]) => void | Promise<void>;
  discard: (docs: Recoverable[]) => void | Promise<void>;
  /** Informe del fallo para copiar (versión, sistema y final del registro); si falta, no se ofrece el botón. */
  report?: () => Promise<string>;
}

const [state, setState] = createSignal<{ docs: Recoverable[]; handlers: RecoveryHandlers } | null>(null);
export const recoverable = state;
export function setRecoverable(docs: Recoverable[], handlers: RecoveryHandlers): void { setState(docs.length ? { docs, handlers } : null); }
export const clearRecoverable = (): void => { setState(null); };
