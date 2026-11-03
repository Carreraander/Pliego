import { createSignal } from "solid-js";

/**
 * Documentos recuperables tras un cierre inesperado. W02 (autoguardado de recuperación) llama a `setRecoverable` al
 * arrancar y pasa qué hacer con ellos; Inicio muestra «Pliego se cerró sin guardar N documentos. [Recuperar] [Descartar]».
 */
export interface Recoverable { id: string; title: string }
export interface RecoveryHandlers { restore: (docs: Recoverable[]) => void | Promise<void>; discard: (docs: Recoverable[]) => void | Promise<void> }

const [state, setState] = createSignal<{ docs: Recoverable[]; handlers: RecoveryHandlers } | null>(null);
export const recoverable = state;
export function setRecoverable(docs: Recoverable[], handlers: RecoveryHandlers): void { setState(docs.length ? { docs, handlers } : null); }
export const clearRecoverable = (): void => { setState(null); };
