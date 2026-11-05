import "./ocr.css";
import { createResource, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { locale } from "../../i18n";
import { Button, Notice, TaskPanel } from "../../ui";
import { errorText, runToolJob } from "../run";
import { languageName, type OcrLanguages } from "./request";
import { oct } from "./strings";

/** Idiomas de OCR: los instalados y los que se pueden descargar (modelos con huella verificada). */
export default function LanguagesPanel(p: { onClose: () => void }) {
  const [langs, { refetch }] = createResource(async () => {
    try { return { ok: await invoke<OcrLanguages>("ocr_languages"), error: null as string | null }; } catch (e) { return { ok: null as OcrLanguages | null, error: errorText(e) }; }
  });
  const data = () => langs()?.ok ?? null;
  const failure = () => langs()?.error ?? null;
  const es = () => locale() === "es";
  const missing = () => (data()?.downloadable ?? []).filter((c) => !data()!.installed.includes(c));
  const download = async (code: string) => {
    await runToolJob({ command: "ocr_download_language", args: { code }, title: oct("ocr.languages.job", { name: languageName(code, es()) }), started: oct("ocr.languages.started") });
  };
  return (
    <TaskPanel title={oct("ocr.languages.title")} onClose={p.onClose} primary={{ label: oct("ocr.languages.refresh"), icon: "refresh", onClick: () => void refetch() }}
      notice={<Show when={failure()}><Notice tone="danger" inline>{oct("ocr.langs.failed", { error: failure()! })}</Notice></Show>}>
      <p class="pl-muted">{oct("ocr.languages.hint")}</p>
      <Show when={data()}>
        <h3 class="pl-label-group">{oct("ocr.languages.installed")}</h3>
        <Show when={data()!.installed.length > 0} fallback={<p class="pl-muted">{oct("ocr.languages.none")}</p>}>
          <ul class="pl-ocr-list"><For each={data()!.installed}>{(c) => <li>{languageName(c, es())}</li>}</For></ul>
        </Show>
        <h3 class="pl-label-group">{oct("ocr.languages.available")}</h3>
        <ul class="pl-ocr-list">
          <For each={missing()}>{(c) => (
            <li class="pl-ocr-row">
              <span>{languageName(c, es())}</span>
              <Button icon="download" onClick={() => void download(c)}>{oct("ocr.languages.download")}</Button>
            </li>
          )}</For>
        </ul>
      </Show>
    </TaskPanel>
  );
}
