/**
 * Servicio de actualizaciones que usa Ajustes. Aquí hay uno **simulado**; W22 (autoactualizaciones) lo sustituye con
 * `setUpdateService(real)` sin tocar la pantalla.
 */
import { createSignal } from "solid-js";

export type UpdateState =
  | { kind: "idle"; lastCheck: Date | null }
  | { kind: "checking" }
  | { kind: "up-to-date"; lastCheck: Date }
  | { kind: "available"; version: string; lastCheck: Date };

/** Estado real que emite el servicio de Rust (`update_status` / `update-state-changed`); `null` fuera de la app. */
export type UpdatePhase =
  | { phase: "not-configured" | "disabled" | "up-to-date" }
  | { phase: "available"; version: string; mandatory: boolean; notes: string | null; notes_url: string | null; can_download: boolean }
  | { phase: "downloading"; done: number; total: number }
  | { phase: "ready"; version: string; mandatory: boolean; notes: string | null; needs_elevation: boolean }
  | { phase: "failed"; message: string; retry_at: string };
export type UpdateSnapshot = UpdatePhase & {
  current_version: string;
  channel: "stable" | "beta";
  mode: "automatic" | "notify-only" | "disabled";
  /** Directiva de la organización: los controles quedan bloqueados. */
  managed: boolean;
  install_kind: "per-user" | "per-machine" | "msi" | "portable";
  last_check: string | null;
  server: string | null;
  history: [string, string][];
  whats_new: string | null;
  whats_new_notes: string | null;
  rollback: [string, string] | null;
  busy: boolean;
};
const [snapshot, setSnapshot] = createSignal<UpdateSnapshot | null>(null);
export const updateSnapshot = snapshot;
export const setUpdateSnapshot = (s: UpdateSnapshot | null): void => { setSnapshot(s); };

export interface UpdateService {
  /** Versión instalada. */
  currentVersion(): string;
  check(): Promise<UpdateState>;
  /** Versiones instaladas en este equipo, la más reciente primero. */
  history(): { version: string; date: string }[];
  /** Tarjeta de novedades tras actualizar (si hay). */
  whatsNew(): { version: string; text: string } | null;
}

export const simulatedUpdates: UpdateService = {
  currentVersion: () => "0.1.0",
  check: async () => { await new Promise((r) => setTimeout(r, 400)); return { kind: "up-to-date", lastCheck: new Date() }; },
  history: () => [{ version: "0.1.0", date: "2026-11-03" }],
  whatsNew: () => null,
};

const [service, setService] = createSignal<UpdateService>(simulatedUpdates);
export const updateService = service;
export const setUpdateService = (s: UpdateService): void => { setService(s); };

const [state, setState] = createSignal<UpdateState>({ kind: "idle", lastCheck: null });
export const updateState = state;
export async function checkForUpdates(): Promise<void> {
  setState({ kind: "checking" });
  try { setState(await service().check()); } catch { setState({ kind: "idle", lastCheck: null }); }
}
