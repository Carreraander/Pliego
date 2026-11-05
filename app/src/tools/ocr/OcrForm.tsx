import "./ocr.css";
import { createResource, createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { locale } from "../../i18n";
import { activeDoc } from "../../state/documents";
import { Checkbox, Field, Notice, Select, TaskPanel, TextInput } from "../../ui";
import { errorText, fileName, runToolJob } from "../run";
import { defaultLanguages, languageName, normalizePages, OCR_DPI, ocrArgs, type OcrLanguages } from "./request";
import { oct } from "./strings";

/** Formulario común de «Reconocer texto» y «Enderezar y limpiar»: la copia se guarda junto al original. */
export function OcrForm(p: { onClose: () => void; variant: "recognize" | "clean" }) {
  const clean = () => p.variant === "clean";
  const doc = activeDoc;
  const [langs] = createResource(async () => {
    try { return { ok: await invoke<OcrLanguages>("ocr_languages"), error: null as string | null }; } catch (e) { return { ok: null as OcrLanguages | null, error: errorText(e) }; }
  });
  const installed = () => (langs()?.ok?.installed ?? []).filter((c) => c !== "osd");
  const [chosen, setChosen] = createSignal<string[] | null>(null);
  const selected = () => chosen() ?? defaultLanguages(installed());
  const toggle = (code: string, on: boolean) => setChosen(on ? [...selected(), code] : selected().filter((c) => c !== code));
  const [pages, setPages] = createSignal("");
  const [force, setForce] = createSignal(false);
  const [dpi, setDpi] = createSignal(300);
  const [deskew, setDeskew] = createSignal(clean());
  const [rotate, setRotate] = createSignal(false);
  const [binarize, setBinarize] = createSignal(false);
  const [denoise, setDenoise] = createSignal(clean());

  const form = () => ({
    path: doc()?.path ?? "", langs: selected(), pages: pages(), force: force(), deskew: deskew(), rotate: rotate(),
    binarize: binarize(), denoise: denoise(), dpi: dpi(), suffix: oct("ocr.suffix"),
  });
  const args = () => ocrArgs(form());
  const run = async () => {
    const a = args();
    const d = doc();
    if (!a || !d) return;
    const id = await runToolJob({ command: "ocr_pdf", args: a, title: oct("ocr.job", { name: fileName(d.path) }), started: oct("ocr.started") });
    if (id !== null) p.onClose();
  };
  const stem = () => fileName(doc()?.path ?? "").replace(/\.pdf$/i, "");
  const failure = () => langs()?.error ?? null;

  return (
    <TaskPanel
      title={oct(clean() ? "ocr.clean.title" : "ocr.recognize.title")} onClose={p.onClose}
      primary={{ label: oct(clean() ? "ocr.clean.run" : "ocr.recognize.run"), onClick: () => void run(), disabled: !doc() || args() === null }}
      notice={<>
        <Show when={!doc()}><Notice tone="warn" inline>{oct("ocr.noDoc")}</Notice></Show>
        <Show when={failure()}><Notice tone="danger" inline>{oct("ocr.langs.failed", { error: failure()! })}</Notice></Show>
        <Show when={!langs.loading && !failure() && installed().length === 0}><Notice tone="warn" inline>{oct("ocr.langs.none")}</Notice></Show>
      </>}
    >
      <Show when={clean()}><p class="pl-muted">{oct("ocr.clean.hint")}</p></Show>
      <fieldset class="pl-ocr-fieldset">
        <legend>{oct("ocr.langs")}</legend>
        <Show when={!langs.loading} fallback={<p class="pl-muted">{oct("ocr.langs.loading")}</p>}>
          <For each={installed()}>{(code) => (
            <Checkbox label={languageName(code, locale() === "es")} checked={selected().includes(code)} onChange={(e) => toggle(code, e.currentTarget.checked)} />
          )}</For>
        </Show>
      </fieldset>
      <Field label={oct("ocr.pages")} hint={oct("ocr.pages.hint")} error={normalizePages(pages()) === null ? oct("ocr.pages.invalid") : undefined}>
        <TextInput value={pages()} onInput={(e) => setPages(e.currentTarget.value)} />
      </Field>
      <Show when={clean()}>
        <Checkbox label={oct("ocr.deskew")} checked={deskew()} onChange={(e) => setDeskew(e.currentTarget.checked)} />
        <Checkbox label={oct("ocr.rotate")} checked={rotate()} onChange={(e) => setRotate(e.currentTarget.checked)} />
        <Checkbox label={oct("ocr.binarize")} checked={binarize()} onChange={(e) => setBinarize(e.currentTarget.checked)} />
        <Checkbox label={oct("ocr.denoise")} checked={denoise()} onChange={(e) => setDenoise(e.currentTarget.checked)} />
      </Show>
      <Checkbox label={oct("ocr.force")} checked={force()} onChange={(e) => setForce(e.currentTarget.checked)} />
      <Field label={oct("ocr.dpi")} hint={oct("ocr.dpi.hint")}>
        <Select value={String(dpi())} onChange={(e) => setDpi(Number(e.currentTarget.value))} options={OCR_DPI.map((d) => ({ value: String(d), label: String(d) }))} />
      </Field>
      <p class="pl-muted">{oct("ocr.note", { name: `${stem()} (${oct("ocr.suffix")}).pdf` })}</p>
    </TaskPanel>
  );
}
