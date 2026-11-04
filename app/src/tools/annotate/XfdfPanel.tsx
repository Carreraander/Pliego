import { createSignal, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { activeDoc } from "../../state/documents";
import { Notice, Radio, RadioGroup, TaskPanel } from "../../ui";
import { fileName, runToolJob } from "../run";
import { at } from "./text";

export default function XfdfPanel(p: { onClose: () => void }) {
  const [dir, setDir] = createSignal<"export" | "import">("export");
  const [file, setFile] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const pick = async () => {
    const f = await invoke<string | null>("pick_xfdf_file", { title: at("annotate.xfdf.pickTitle") });
    if (f) setFile(f);
  };
  const run = async () => {
    const doc = activeDoc();
    if (!doc) return;
    setError(null);
    if (dir() === "import" && !file()) { setError(at("annotate.xfdf.noFile")); return; }
    const exporting = dir() === "export";
    const id = await runToolJob({
      command: exporting ? "export_comments_xfdf" : "import_comments_xfdf",
      args: exporting
        ? { path: doc.path, suffix: at("annotate.xfdf.suffixExport") }
        : { path: doc.path, xfdf: file(), suffix: at("annotate.xfdf.suffixImport") },
      title: at(exporting ? "annotate.xfdf.exportJob" : "annotate.xfdf.importJob", { name: fileName(doc.path) }),
      started: at("annotate.xfdf.started"),
    });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={at("annotate.xfdf.title")} onClose={p.onClose}
      primary={{ label: dir() === "export" ? at("annotate.xfdf.export") : at("annotate.xfdf.import"), icon: "arrow-left-right", onClick: () => void run() }}
      secondary={dir() === "import" ? [{ label: at("annotate.xfdf.pick"), icon: "file-plus", onClick: () => void pick() }] : undefined}
      estimate={dir() === "export" ? at("annotate.xfdf.exportHint") : (file() ? fileName(file()!) : at("annotate.xfdf.importHint"))}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <RadioGroup legend={at("annotate.xfdf.direction")} value={dir()} onChange={(v) => setDir(v as "export" | "import")}>
        <Radio value="export" label={at("annotate.xfdf.export")} />
        <Radio value="import" label={at("annotate.xfdf.import")} />
      </RadioGroup>
    </TaskPanel>
  );
}
