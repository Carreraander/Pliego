import { createSignal, Show } from "solid-js";
import { api } from "../../api";
import { activeDoc } from "../../state/documents";
import { Button, Checkbox, Field, Notice, TaskPanel, toast } from "../../ui";
import { openSideBySide } from "./open";
import { errorText, fileName, runToolJob } from "../run";
import { compareArgs, sameFile } from "./request";
import { cmt } from "./strings";

/** Comparar: el documento abierto (original) con otro PDF (versión nueva); guarda un informe PDF junto al original. */
export default function ComparePanel(p: { onClose: () => void }) {
  const doc = activeDoc;
  const [other, setOther] = createSignal<string | null>(null);
  const [text, setText] = createSignal(true);
  const [visual, setVisual] = createSignal(true);
  const pick = async () => { const f = await api.pickPdfFiles(cmt("compare.other")); if (f[0]) setOther(f[0]); };
  const args = () => compareArgs({ path: doc()?.path ?? "", other: other(), text: text(), visual: visual(), label: cmt("compare.suffix") });
  const same = () => !!doc() && !!other() && sameFile(doc()!.path, other()!);
  const run = async () => {
    const a = args();
    const d = doc();
    if (!a || !d) return;
    const id = await runToolJob({ command: "compare_pdfs", args: a, title: cmt("compare.job", { name: fileName(d.path) }), started: cmt("compare.started") });
    if (id !== null) p.onClose();
  };
  const [busy, setBusy] = createSignal(false);
  const sideBySide = async () => {
    const d = doc();
    if (!d || !other() || busy() || (!text() && !visual()) || same()) return;
    setBusy(true);
    try {
      await openSideBySide({ doc: d, other: other()!, text: text(), visual: visual() });
      p.onClose();
    } catch (e) {
      toast({ tone: "danger", text: `${cmt("view.failed")}: ${errorText(e)}` });
    } finally {
      setBusy(false);
    }
  };
  const stem = () => fileName(doc()?.path ?? "").replace(/\.pdf$/i, "");
  return (
    <TaskPanel title={cmt("compare.title")} onClose={p.onClose} primary={{ label: cmt("compare.sideBySide"), icon: "columns-2", onClick: () => void sideBySide(), disabled: args() === null || busy() }}
      secondary={[{ label: cmt("compare.run"), onClick: () => void run(), disabled: args() === null || busy() }]}
      progress={busy() ? { value: null, label: cmt("compare.sideBySide.busy") } : undefined}
      notice={<>
        <Show when={!doc()}><Notice tone="warn" inline>{cmt("compare.noDoc")}</Notice></Show>
        <Show when={same()}><Notice tone="warn" inline>{cmt("compare.same")}</Notice></Show>
      </>}>
      <Field label={cmt("compare.other")} hint={other() ?? cmt("compare.other.none")}>
        <Button icon="file-plus" onClick={() => void pick()}>{other() ? fileName(other()!) : cmt("compare.other.pick")}</Button>
      </Field>
      <p class="pl-muted">{cmt("compare.other.hint")}</p>
      <Checkbox label={cmt("compare.text")} checked={text()} onChange={(e) => setText(e.currentTarget.checked)} />
      <Checkbox label={cmt("compare.visual")} checked={visual()} onChange={(e) => setVisual(e.currentTarget.checked)} />
      <p class="pl-muted">{cmt("compare.sideBySide.hint")}</p>
      <p class="pl-muted">{cmt("compare.note", { name: `${stem()} (${cmt("compare.suffix")}).pdf` })}</p>
    </TaskPanel>
  );
}
