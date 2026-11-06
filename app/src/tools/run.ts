/**
 * Ayudante común de las herramientas que lanzan un trabajo: invoca el comando (que devuelve un `JobId`), lo da de alta
 * en el centro de tareas con su título y un «Reintentar», y avisa con un toast. Lo reutilizan Comprimir, OCR, Convertir…
 */
import { invoke } from "@tauri-apps/api/core";
import { trackJob } from "../jobs";
import { withPassword } from "../state/passwords";
import { engineText } from "../i18n/engine";
import { toast } from "../ui";

export { formatBytes } from "./format";

export interface RunToolJob {
  command: string;
  args: Record<string, unknown>;
  /** Título en el centro de tareas («Comprimir contrato.pdf»). */
  title: string;
  /** Texto del toast al empezar. */
  started: string;
}

/** Lanza el trabajo y devuelve su id. Si el comando falla antes de empezar (ruta no válida…) lo muestra en un toast. */
export async function runToolJob(job: RunToolJob): Promise<number | null> {
  try {
    const id = await invoke<number>(job.command, withPassword(job.args));
    trackJob(id, { title: job.title, retry: () => void runToolJob(job) });
    toast({ tone: "info", text: job.started });
    return id;
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
    return null;
  }
}

export function errorText(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return engineText(String((e as { message: unknown }).message));
  return engineText(String(e));
}

export const fileName = (path: string): string => path.split(/[\\/]/).pop() ?? path;
