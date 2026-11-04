import { createSignal, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { isApiError } from "../../api";
import { trackJob } from "../../jobs";
import { activeDoc } from "../../state/documents";
import { Notice, Radio, RadioGroup, TaskPanel, toast } from "../../ui";
import { at } from "./text";

const base = (p: string) => p.split(/[\\/]/).pop() ?? p;

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
    try {
      if (dir() === "export") {
        const id = await invoke<number>("export_comments_xfdf", { path: doc.path });
        trackJob(id, { title: at("annotate.xfdf.exportJob", { name: doc.title }), retry: () => void run() });
      } else {
        if (!file()) { setError(at("annotate.xfdf.noFile")); return; }
        const id = await invoke<number>("import_comments_xfdf", { path: doc.path, xfdf: file() });
        trackJob(id, { title: at("annotate.xfdf.importJob", { name: doc.title }), retry: () => void run() });
      }
      toast({ tone: "info", text: at("annotate.xfdf.started") });
      p.onClose();
    } catch (e) {
      setError(isApiError(e) ? e.message : String(e));
    }
  };

  return (
    <TaskPanel
      title={at("annotate.xfdf.title")} onClose={p.onClose}
      primary={{ label: dir() === "export" ? at("annotate.xfdf.export") : at("annotate.xfdf.import"), icon: "arrow-left-right", onClick: () => void run() }}
      secondary={dir() === "import" ? [{ label: at("annotate.xfdf.pick"), icon: "file-plus", onClick: () => void pick() }] : undefined}
      estimate={dir() === "export" ? at("annotate.xfdf.exportHint") : (file() ? base(file()!) : at("annotate.xfdf.importHint"))}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <RadioGroup legend={at("annotate.xfdf.direction")} value={dir()} onChange={(v) => setDir(v as "export" | "import")}>
        <Radio value="export" label={at("annotate.xfdf.export")} />
        <Radio value="import" label={at("annotate.xfdf.import")} />
      </RadioGroup>
    </TaskPanel>
  );
}
