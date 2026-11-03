/**
 * Estado del centro de tareas: escucha `job-progress` y `job-finished` (los emite `commands/jobs.rs`).
 * Un flujo que lanza una operación larga llama a `trackJob(jobId, { title })` con el `JobId` que le devolvió su comando.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { createMemo, createSignal } from "solid-js";
import { isTauri } from "../shell/platform";

export interface JobReport { outputs: string[]; warnings: string[]; summary: string }
export interface JobFailure { code: string; message: string; cancelled: boolean }
export type JobStatus = "running" | "done" | "error" | "cancelled";

export interface Job {
  id: number;
  title: string;
  status: JobStatus;
  done: number;
  total: number;
  label: string;
  report?: JobReport;
  error?: string;
  /** Vuelve a lanzar la tarea (solo si quien la creó lo indicó). */
  retry?: () => void;
  startedAt: number;
}

const [list, setList] = createSignal<Job[]>([]);
export const jobs = list;
export const runningJobs = createMemo(() => list().filter((j) => j.status === "running"));

type JobProgressEvent = { job_id: number; done: number; total: number; label: string };
type JobFinishedEvent = { job_id: number; result: JobReport | null; error: JobFailure | null };

function upsert(id: number, patch: (j: Job) => Partial<Job>): void {
  setList((all) => {
    const i = all.findIndex((j) => j.id === id);
    const base: Job = i >= 0 ? all[i]! : { id, title: "", status: "running", done: 0, total: 0, label: "", startedAt: Date.now() };
    const next = { ...base, ...patch(base) };
    return i >= 0 ? all.map((j, k) => (k === i ? next : j)) : [next, ...all];
  });
}

/** Da título (y, si se quiere, un modo de reintentar) a un trabajo recién lanzado. */
export function trackJob(id: number, info: { title: string; retry?: () => void }): void {
  upsert(id, () => ({ title: info.title, retry: info.retry }));
}

export function onProgress(e: JobProgressEvent): void {
  upsert(e.job_id, (j) => ({ done: e.done, total: e.total, label: e.label, title: j.title || e.label }));
}

type FinishListener = (job: Job) => void;
const finishListeners = new Set<FinishListener>();
/** Avisa cuando un trabajo termina (notificación de Windows, apertura del resultado…). */
export const onJobFinished = (f: FinishListener): (() => void) => { finishListeners.add(f); return () => finishListeners.delete(f); };

export function onFinished(e: JobFinishedEvent): void {
  const status: JobStatus = e.error ? (e.error.cancelled ? "cancelled" : "error") : "done";
  upsert(e.job_id, () => ({ status, report: e.result ?? undefined, error: e.error?.message }));
  const job = list().find((j) => j.id === e.job_id);
  if (job) finishListeners.forEach((f) => f(job));
}

export async function cancelJob(id: number): Promise<void> {
  if (isTauri()) await invoke<boolean>("cancel_job", { jobId: id });
}
export const clearFinished = (): void => { setList((all) => all.filter((j) => j.status === "running")); };
export const dismissJob = (id: number): void => { setList((all) => all.filter((j) => j.id !== id)); };

/** Empieza a escuchar los eventos de Tauri. */
export async function listenJobs(): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  const offs = await Promise.all([
    listen<JobProgressEvent>("job-progress", (e) => onProgress(e.payload)),
    listen<JobFinishedEvent>("job-finished", (e) => onFinished(e.payload)),
  ]);
  return () => offs.forEach((off) => off());
}

/** Solo para pruebas. */
export function resetJobs(): void { setList([]); finishListeners.clear(); }
