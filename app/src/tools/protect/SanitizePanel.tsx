import { createSignal, For, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { Checkbox, Notice, TaskPanel } from "../../ui";
import { runToolJob } from "../run";
import { activeDoc } from "../../state/documents";
import { baseName, buildSanitizeRequest, SANITIZE_ALL, SANITIZE_OPTIONS, type SanitizeOptions } from "./request";
import { pt } from "./strings";

export default function SanitizePanel(p: { onClose?: () => void }) {
  const [options, setOptions] = createStore<SanitizeOptions>({ ...SANITIZE_ALL });
  const [error, setError] = createSignal<string | null>(null);

  const run = async () => {
    const doc = activeDoc();
    if (!doc) return;
    const built = buildSanitizeRequest(doc.path, "", { ...options });
    if (!built.ok) { setError(pt(built.error)); return; }
    setError(null);
    const id = await runToolJob({
      command: "protect_sanitize", args: { request: built.request },
      title: pt("sanitize.job", { name: baseName(doc.path) }), started: pt("sanitize.started"),
    });
    if (id !== null) p.onClose?.();
  };

  return (
    <TaskPanel
      title={pt("sanitize.panel")} onClose={p.onClose}
      primary={{ label: pt("sanitize.run"), onClick: () => void run() }}
      notice={<Show when={error()}><Notice tone="danger">{error()}</Notice></Show>}
      estimate={pt("protect.copyNote")}
    >
      <p class="pl-muted">{pt("sanitize.intro")}</p>
      <For each={SANITIZE_OPTIONS}>
        {(k) => <Checkbox label={pt(`sanitize.${k}`)} checked={options[k]} onChange={(e) => setOptions(k, e.currentTarget.checked)} />}
      </For>
    </TaskPanel>
  );
}
