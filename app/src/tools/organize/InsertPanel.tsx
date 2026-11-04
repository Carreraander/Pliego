import { createSignal, Show } from "solid-js";
import { api } from "../../api";
import { activeDoc, currentPageIndex } from "../../state/documents";
import { Button, Field, Notice, Radio, RadioGroup, TaskPanel, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
import { insertArgs } from "./request";
import { ot } from "./text";

/** Insertar páginas en blanco o de otro PDF antes de una página; guarda una copia. */
export default function InsertPanel(p: { onClose: () => void }) {
  const total = () => activeDoc()?.sizes.length ?? 0;
  const [kind, setKind] = createSignal<"blank" | "file">("blank");
  const [count, setCount] = createSignal("1");
  const [source, setSource] = createSignal<string | null>(null);
  const [pages, setPages] = createSignal("");
  const [where, setWhere] = createSignal(String(currentPageIndex() + 1));
  const [error, setError] = createSignal<string | null>(null);

  const pick = async () => { const f = await api.pickPdfFiles(); if (f[0]) setSource(f[0]); };
  const args = () => insertArgs({ path: activeDoc()?.path ?? "", total: total(), kind: kind(), count: count(), source: source(), pages: pages(), where: where(), suffix: ot("organize.insert.suffix") });

  const run = async () => {
    const a = args();
    const d = activeDoc();
    if (!a || !d) { setError(ot("organize.invalid")); return; }
    const id = await runToolJob({ command: "insert_pages", args: a, title: ot("organize.insert.job", { name: fileName(d.path) }), started: ot("organize.insert.started") });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={ot("organize.insert.title")} onClose={p.onClose}
      primary={{ label: ot("organize.insert.run"), icon: "file-plus", disabled: args() === null, onClick: () => void run() }}
      estimate={ot("organize.insert.hint")}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <RadioGroup legend={ot("organize.insert.from")} value={kind()} onChange={(v) => { setKind(v as "blank" | "file"); setError(null); }}>
        <Radio value="blank" label={ot("organize.insert.blank")} />
        <Radio value="file" label={ot("organize.insert.file")} />
      </RadioGroup>
      <Show when={kind() === "blank"}>
        <Field label={ot("organize.insert.count")}><TextInput type="number" min="1" value={count()} onInput={(e) => setCount(e.currentTarget.value)} /></Field>
      </Show>
      <Show when={kind() === "file"}>
        <Field label={ot("organize.insert.source")} hint={source() ?? undefined}>
          <Button icon="file-plus" onClick={() => void pick()}>{source() ? fileName(source()!) : ot("organize.insert.choose")}</Button>
        </Field>
        <Field label={ot("organize.insert.pages")} hint={ot("organize.insert.pagesHint")}><TextInput value={pages()} onInput={(e) => setPages(e.currentTarget.value)} /></Field>
      </Show>
      <Field label={ot("organize.insert.where")} hint={ot("organize.insert.whereHint", { n: total() + 1 })}>
        <TextInput type="number" min="1" max={total() + 1} value={where()} onInput={(e) => setWhere(e.currentTarget.value)} />
      </Field>
    </TaskPanel>
  );
}
