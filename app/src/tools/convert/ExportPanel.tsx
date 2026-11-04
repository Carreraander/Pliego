import { createSignal, Show } from "solid-js";
import { activeDoc } from "../../state/documents";
import { Field, Notice, Select, TaskPanel, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
import { exportArgs, normalizePages, optionalNumber, usesDpi, type ExportFormat } from "./request";
import { ct } from "./strings";

/** Exportar a imágenes (PNG, JPEG, TIFF, SVG, las incrustadas) o a texto/HTML. Guarda junto al original. */
export default function ExportPanel(p: { onClose: () => void }) {
  const [format, setFormat] = createSignal<ExportFormat>("png");
  const [pages, setPages] = createSignal("");
  const [dpi, setDpi] = createSignal("");
  const [quality, setQuality] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  const doc = activeDoc;
  const run = async () => {
    const d = doc();
    const range = normalizePages(pages());
    if (!d) return;
    if (range === null) return setError(ct("convert.pages.bad"));
    setError(null);
    const f = format();
    const label = f === "txt" || f === "html" ? ct("convert.export.label.text") : ct("convert.export.label.images");
    const id = await runToolJob({
      command: "export_pdf",
      args: exportArgs(d.path, f, range, optionalNumber(dpi(), 36, 1200), optionalNumber(quality(), 1, 100), label),
      title: ct("convert.export.job", { name: fileName(d.path) }),
      started: ct("convert.export.started"),
    });
    if (id !== null) p.onClose();
  };
  const formats: ExportFormat[] = ["png", "jpg", "tiff", "svg", "images", "txt", "html"];
  return (
    <TaskPanel title={ct("convert.export.title")} onClose={p.onClose} primary={{ label: ct("convert.export.run"), onClick: run, disabled: !doc() }}
      notice={<Show when={!doc() || error()}><Notice tone={error() ? "danger" : "warn"} inline>{error() ?? ct("convert.noDoc")}</Notice></Show>}>
      <Field label={ct("convert.export.format")}>
        <Select value={format()} onChange={(e) => setFormat(e.currentTarget.value as ExportFormat)} options={formats.map((f) => ({ value: f, label: ct(`convert.export.${f}`) }))} />
      </Field>
      <Field label={ct("convert.pages")} hint={ct("convert.pages.hint")}>
        <TextInput value={pages()} onInput={(e) => setPages(e.currentTarget.value)} />
      </Field>
      <Show when={usesDpi(format())}>
        <Field label={ct("convert.export.dpi")} hint={ct("convert.export.dpi.hint")}>
          <TextInput inputmode="decimal" value={dpi()} onInput={(e) => setDpi(e.currentTarget.value)} />
        </Field>
      </Show>
      <Show when={format() === "jpg"}>
        <Field label={ct("convert.export.quality")} hint={ct("convert.export.quality.hint")}>
          <TextInput inputmode="numeric" value={quality()} onInput={(e) => setQuality(e.currentTarget.value)} />
        </Field>
      </Show>
      <p class="pl-muted">{ct("convert.export.note")}</p>
    </TaskPanel>
  );
}
