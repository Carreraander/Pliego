import "./batch.css";
import { createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { api } from "../../api";
import { Button, Checkbox, IconButton, Notice, TaskPanel } from "../../ui";
import { errorText, fileName, runToolJob } from "../run";
import { stepLabel, type SavedAction } from "./actionSteps";
import { bt } from "./strings";

/** Acciones en lote: elegir una acción guardada, las entradas y la carpeta de salida, y ejecutarla como trabajo. */
export default function ActionsPanel(p: { onClose: () => void }) {
  const [path, setPath] = createSignal<string | null>(null);
  const [action, setAction] = createSignal<SavedAction | null>(null);
  const [inputs, setInputs] = createSignal<string[]>([]);
  const [output, setOutput] = createSignal<string | null>(null);
  const [resume, setResume] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const pickAction = async () => {
    const f = await api.pickActionFile(bt("batch.actions.title"));
    if (!f) return;
    setError(null);
    try {
      setAction(await invoke<SavedAction>("actions_load", { path: f }));
      setPath(f);
    } catch (e) {
      setAction(null); setPath(null);
      setError(bt("batch.actions.loadFailed", { error: errorText(e) }));
    }
  };
  const add = (more: string[]) => setInputs((all) => [...all, ...more.filter((x) => !all.includes(x))]);
  const addFiles = async () => add(await api.pickPdfFiles());
  const addFolder = async () => { const f = await api.pickFolder(bt("batch.actions.inputs")); if (f) add([f]); };
  const pickOutput = async () => setOutput((await api.pickFolder(bt("batch.actions.output"))) ?? output());

  const run = async () => {
    const a = action(), f = path();
    if (!a || !f || inputs().length === 0) return;
    const id = await runToolJob({
      command: "actions_run",
      args: { action: f, inputs: inputs(), output: output(), resume: resume(), suffix: bt("batch.actions.suffix") },
      title: bt("batch.actions.job", { name: a.name || bt("batch.actions.unnamed"), n: inputs().length }),
      started: bt("batch.actions.started"),
    });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={bt("batch.actions.title")} onClose={p.onClose} estimate={bt("batch.actions.hint")}
      primary={{ label: bt("batch.actions.run"), icon: "list-checks", disabled: !action() || inputs().length === 0, onClick: () => void run() }}
      secondary={[
        { label: bt("batch.actions.pick"), icon: "list-checks", onClick: () => void pickAction() },
        { label: bt("batch.actions.addFiles"), icon: "file-plus", onClick: () => void addFiles() },
        { label: bt("batch.actions.addFolder"), icon: "folders", onClick: () => void addFolder() },
      ]}
      notice={<Show when={error() || (action() && inputs().length === 0)}>
        <Notice tone={error() ? "danger" : "info"} inline>{error() ?? bt("batch.actions.noInputs")}</Notice>
      </Show>}
    >
      <Show when={action()} fallback={<p>{bt("batch.actions.none")}</p>}>
        {(a) => (
          <>
            <p class="pl-field__label">{a().name || bt("batch.actions.unnamed")}</p>
            <Show when={a().description}><p>{a().description}</p></Show>
            <p class="pl-field__label">{bt("batch.actions.steps")}</p>
            <ol class="pl-index-hits"><For each={a().steps}>{(s) => <li>{stepLabel(s)}</li>}</For></ol>
          </>
        )}
      </Show>
      <p class="pl-field__label">{bt("batch.actions.inputs")}</p>
      <ul class="pl-index-hits">
        <For each={inputs()}>{(f) => (
          <li>
            <span title={f}>{fileName(f)}</span>
            <IconButton icon="x" label={bt("batch.actions.remove")} size="sm" onClick={() => setInputs((all) => all.filter((x) => x !== f))} />
          </li>
        )}</For>
      </ul>
      <p class="pl-field__label">{bt("batch.actions.output")}</p>
      <p><Button onClick={() => void pickOutput()}>{bt("batch.actions.outputPick")}</Button> <small>{output() ?? bt("batch.actions.outputDefault")}</small></p>
      <Checkbox label={bt("batch.actions.resume")} checked={resume()} onChange={(e) => setResume(e.currentTarget.checked)} />
    </TaskPanel>
  );
}
