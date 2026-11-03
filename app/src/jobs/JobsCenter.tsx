import { invoke } from "@tauri-apps/api/core";
import { createSignal, For, Show } from "solid-js";
import { Button, Icon, IconButton, Popover, ProgressBar } from "../ui";
import { isTauri } from "../shell/platform";
import { cancelJob, clearFinished, dismissJob, jobs, runningJobs, type Job } from "./state";
import { jt } from "./strings";
import "./jobs.css";

const [open, setOpen] = createSignal(false);
let openResult: ((path: string) => void) | null = null;
/** La app dice cómo abrir un PDF resultado en una pestaña. */
export const setResultOpener = (f: ((path: string) => void) | null): void => { openResult = f; };

async function reveal(path: string): Promise<void> {
  if (isTauri()) await invoke("reveal_path", { path });
}

function JobItem(props: { job: Job }) {
  const j = () => props.job;
  const out = () => j().report?.outputs[0];
  return (
    <li class="pl-job" data-status={j().status}>
      <div class="pl-job__head">
        <Icon name={j().status === "done" ? "check-circle" : j().status === "running" ? "refresh" : "x-circle"} size={16} />
        <span class="pl-job__title">{j().title || j().label}</span>
        <Show when={j().status !== "running"}><IconButton icon="x" label={jt("jobs.dismiss")} onClick={() => dismissJob(j().id)} /></Show>
      </div>
      <Show when={j().status === "running"}>
        <ProgressBar value={j().total > 0 ? j().done / j().total : null} label={j().label || jt("jobs.progress", { done: j().done, total: j().total })} onCancel={() => void cancelJob(j().id)} />
      </Show>
      <Show when={j().status === "done"}>
        <p class="pl-job__text">{j().report?.summary || jt("jobs.done")}</p>
        <div class="pl-job__actions">
          <Show when={out()}><Button onClick={() => openResult?.(out()!)}>{jt("jobs.open")}</Button><Button variant="ghost" onClick={() => void reveal(out()!)}>{jt("jobs.reveal")}</Button></Show>
        </div>
      </Show>
      <Show when={j().status === "cancelled"}><p class="pl-job__text">{jt("jobs.cancelled")}</p></Show>
      <Show when={j().status === "error"}>
        <p class="pl-job__text pl-job__text--error" role="alert">{j().error}</p>
        <Show when={j().retry}><div class="pl-job__actions"><Button onClick={() => j().retry!()}>{jt("jobs.retry")}</Button></div></Show>
      </Show>
    </li>
  );
}

/** Contenido del popover: en curso, terminadas y fallidas (§3.6). */
export function JobsList() {
  const group = (status: (s: Job["status"]) => boolean) => jobs().filter((j) => status(j.status));
  return (
    <div class="pl-jobs">
      <div class="pl-jobs__head"><h2>{jt("jobs.title")}</h2><IconButton icon="x" label={jt("jobs.close")} onClick={() => setOpen(false)} /></div>
      <Show when={jobs().length} fallback={<p class="pl-jobs__empty">{jt("jobs.none")}</p>}>
        <For each={[
          { key: "jobs.running", items: group((s) => s === "running") },
          { key: "jobs.failed", items: group((s) => s === "error") },
          { key: "jobs.finished", items: group((s) => s === "done" || s === "cancelled") },
        ]}>{(g) => (
          <Show when={g.items.length}>
            <h3 class="pl-label-group">{jt(g.key)}</h3>
            <ul class="pl-jobs__list"><For each={g.items}>{(j) => <JobItem job={j} />}</For></ul>
          </Show>
        )}</For>
        <Show when={jobs().some((j) => j.status !== "running")}><Button variant="ghost" onClick={clearFinished}>{jt("jobs.clear")}</Button></Show>
      </Show>
    </div>
  );
}

export const openJobsCenter = (): void => { setOpen(true); };

/** Elemento de la barra de estado: resumen de tareas y popover con el centro de tareas. */
export function JobsStatus() {
  let anchor!: HTMLSpanElement;
  const label = () => {
    const n = runningJobs().length;
    return n === 0 ? jt("jobs.status.idle") : n === 1 ? jt("jobs.status.one") : jt("jobs.status.many", { n });
  };
  return (
    <span ref={anchor} class="pl-jobs__anchor">
      <Icon name="refresh" size={14} /> <span>{label()}</span>
      <Popover anchor={anchor} open={open()} onClose={() => setOpen(false)} placement="top-end" label={jt("jobs.title")} class="pl-jobs__popover"><JobsList /></Popover>
    </span>
  );
}

export { open as jobsCenterOpen, setOpen as setJobsCenterOpen };
