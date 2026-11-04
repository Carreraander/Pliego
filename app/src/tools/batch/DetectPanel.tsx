import { Show } from "solid-js";
import { activeDoc } from "../../state/documents";
import { Notice, TaskPanel } from "../../ui";
import { fileName, runToolJob } from "../run";
import { bt } from "./strings";

/** Detectar campos: crea los campos de formulario que encuentra y guarda una copia junto al original. */
export default function DetectPanel(p: { onClose: () => void }) {
  const run = async () => {
    const d = activeDoc();
    if (!d) return;
    const id = await runToolJob({
      command: "form_detect", args: { path: d.path, suffix: bt("batch.detect.suffix") },
      title: bt("batch.detect.job", { name: fileName(d.path) }), started: bt("batch.detect.started"),
    });
    if (id !== null) p.onClose();
  };
  return (
    <TaskPanel
      title={bt("batch.detect.title")} onClose={p.onClose} estimate={bt("batch.detect.hint")}
      primary={{ label: bt("batch.detect.run"), icon: "scan-search", disabled: !activeDoc(), onClick: () => void run() }}
      notice={<Show when={!activeDoc()}><Notice tone="warn" inline>{bt("batch.detect.noDoc")}</Notice></Show>}
    >
      <p>{bt("batch.detect.hint")}</p>
    </TaskPanel>
  );
}
