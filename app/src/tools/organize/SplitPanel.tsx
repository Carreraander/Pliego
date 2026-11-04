import { createSignal, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { isApiError } from "../../api";
import { trackJob } from "../../jobs";
import { activeDoc } from "../../state/documents";
import { Field, Notice, Select, TaskPanel, TextInput, toast } from "../../ui";
import { parseRanges } from "./ranges";
import { ot } from "./text";

type Mode = "every_n" | "ranges" | "bookmarks" | "max_bytes";

export default function SplitPanel(p: { onClose: () => void }) {
  const [mode, setMode] = createSignal<Mode>("every_n");
  const [n, setN] = createSignal("1");
  const [ranges, setRanges] = createSignal("");
  const [mb, setMb] = createSignal("10");
  const [error, setError] = createSignal<string | null>(null);

  const rule = () => {
    const pages = activeDoc()?.sizes.length ?? 0;
    switch (mode()) {
      case "every_n": { const v = Number(n()); return Number.isInteger(v) && v >= 1 ? { kind: "every_n", n: v } : null; }
      case "ranges": { const r = parseRanges(ranges(), pages); return r ? { kind: "ranges", ranges: r } : null; }
      case "bookmarks": return { kind: "bookmarks" };
      case "max_bytes": { const v = Number(mb()); return v > 0 ? { kind: "max_bytes", bytes: Math.round(v * 1024 * 1024) } : null; }
    }
  };

  const run = async () => {
    const doc = activeDoc();
    const r = rule();
    if (!doc || !r) { setError(ot("organize.split.invalid")); return; }
    try {
      const id = await invoke<number>("split_pdf", { path: doc.path, rule: r });
      trackJob(id, { title: ot("organize.split.job", { name: doc.title }), retry: () => void run() });
      toast({ tone: "info", text: ot("organize.split.started") });
      p.onClose();
    } catch (e) {
      setError(isApiError(e) ? e.message : String(e));
    }
  };

  return (
    <TaskPanel
      title={ot("organize.split.title")} onClose={p.onClose}
      primary={{ label: ot("organize.split.run"), icon: "scissors", disabled: rule() === null, onClick: () => void run() }}
      estimate={ot("organize.split.hint")}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <Field label={ot("organize.split.by")}>
        <Select value={mode()} onChange={(e) => { setMode(e.currentTarget.value as Mode); setError(null); }} options={[
          { value: "every_n", label: ot("organize.split.every") }, { value: "ranges", label: ot("organize.split.ranges") },
          { value: "bookmarks", label: ot("organize.split.bookmarks") }, { value: "max_bytes", label: ot("organize.split.size") },
        ]} />
      </Field>
      <Show when={mode() === "every_n"}>
        <Field label={ot("organize.split.n")}><TextInput type="number" min="1" value={n()} onInput={(e) => setN(e.currentTarget.value)} /></Field>
      </Show>
      <Show when={mode() === "ranges"}>
        <Field label={ot("organize.split.rangesLabel")} hint={ot("organize.split.rangesHint")}><TextInput value={ranges()} onInput={(e) => setRanges(e.currentTarget.value)} /></Field>
      </Show>
      <Show when={mode() === "max_bytes"}>
        <Field label={ot("organize.split.mb")}><TextInput type="number" min="1" value={mb()} onInput={(e) => setMb(e.currentTarget.value)} /></Field>
      </Show>
    </TaskPanel>
  );
}
