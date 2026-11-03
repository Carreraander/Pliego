import { registerStatusItem } from "../shell";
import { jt } from "./strings";
import { JobsStatus, openJobsCenter, setResultOpener } from "./JobsCenter";
import { listenJobs, onJobFinished } from "./state";

export { trackJob, jobs, cancelJob, type Job } from "./state";
export { openJobsCenter, setResultOpener, JobsList } from "./JobsCenter";

/** Notificación de Windows al terminar una tarea con la ventana en segundo plano (API Notification de WebView2). */
function notifyFinished(title: string, ok: boolean, body: string): void {
  if (typeof Notification === "undefined" || document.hasFocus()) return;
  try {
    if (Notification.permission === "granted") new Notification(`${ok ? jt("jobs.notify.done") : jt("jobs.notify.error")}: ${title}`, { body });
    else if (Notification.permission === "default") void Notification.requestPermission();
  } catch { /* sin notificaciones */ }
}

/** Escucha los eventos de trabajos, monta el resumen en la barra de estado y las notificaciones. Devuelve cómo pararlo. */
export async function installJobs(openPdf: (path: string) => void): Promise<() => void> {
  setResultOpener(openPdf);
  const offs = [
    registerStatusItem({ id: "jobs.status", slot: "center", order: 10, component: JobsStatus, onClick: openJobsCenter, label: jt("jobs.title"), tone: undefined }),
    onJobFinished((job) => { if (job.status === "done" || job.status === "error") notifyFinished(job.title, job.status === "done", job.report?.summary ?? job.error ?? ""); }),
    await listenJobs(),
  ];
  return () => { offs.forEach((off) => off()); setResultOpener(null); };
}
