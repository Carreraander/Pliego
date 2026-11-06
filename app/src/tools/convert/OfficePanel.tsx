import { createSignal, Show } from "solid-js";
import { locale } from "../../i18n";
import { activeDoc } from "../../state/documents";
import { Checkbox, Field, Notice, Radio, RadioGroup, TaskPanel, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
import { normalizePages, officeArgs, type OfficeFormat } from "./request";
import { ct } from "./strings";

/** Exportar a Word, Excel o PowerPoint (beta). Guarda `<nombre> (Office).docx|xlsx|pptx` junto al original. */
export default function OfficePanel(p: { onClose: () => void }) {
  const [format, setFormat] = createSignal<OfficeFormat>("docx");
  const [pages, setPages] = createSignal("");
  const [fail, setFail] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const doc = activeDoc;
  const run = async () => {
    const d = doc();
    const range = normalizePages(pages());
    if (!d) return;
    if (range === null) return setError(ct("convert.pages.bad"));
    setError(null);
    const id = await runToolJob({
      command: "office_export_pdf",
      args: officeArgs(d.path, format(), range, fail(), ct("convert.office.label"), locale()),
      title: ct("convert.office.job", { name: fileName(d.path) }),
      started: ct("convert.office.started"),
    });
    if (id !== null) p.onClose();
  };
  return (
    <TaskPanel title={ct("convert.office.title")} onClose={p.onClose} primary={{ label: ct("convert.office.run"), onClick: run, disabled: !doc() }}
      notice={<Show when={!doc() || error()}><Notice tone={error() ? "danger" : "warn"} inline>{error() ?? ct("convert.noDoc")}</Notice></Show>}>
      <RadioGroup legend={ct("convert.office.format")} value={format()} onChange={(v) => setFormat(v as OfficeFormat)}>
        <Radio value="docx" label={ct("convert.office.docx")} />
        <Radio value="xlsx" label={ct("convert.office.xlsx")} />
        <Radio value="pptx" label={ct("convert.office.pptx")} />
      </RadioGroup>
      <Field label={ct("convert.pages")} hint={ct("convert.pages.hint")}>
        <TextInput value={pages()} onInput={(e) => setPages(e.currentTarget.value)} />
      </Field>
      <Checkbox label={ct("convert.office.scanned")} checked={fail()} onChange={(e) => setFail(e.currentTarget.checked)} />
      <p class="pl-muted">{ct("convert.office.beta")}</p>
    </TaskPanel>
  );
}
