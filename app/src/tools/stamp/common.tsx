import "./stamp.css";
import { createSignal, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { api, isApiError } from "../../api";
import { trackJob } from "../../jobs";
import { activeDoc } from "../../state/documents";
import { Button, Field, Notice, TextInput, toast } from "../../ui";
import { st } from "./text";
import { pagesOrAll, validPages } from "./util";

export const FONTS = () => [
  { value: "Helvetica", label: st("stamp.font.helvetica") },
  { value: "Times-Roman", label: st("stamp.font.times") },
  { value: "Courier", label: st("stamp.font.courier") },
];

/** Campo «Páginas» común a las cuatro herramientas. */
export function PagesField(p: { value: string; onInput: (v: string) => void }) {
  return (
    <Field label={st("stamp.pages")} hint={st("stamp.pages.hint")} error={validPages(p.value) ? undefined : st("stamp.pages.invalid")}>
      <TextInput value={p.value} onInput={(e) => p.onInput(e.currentTarget.value)} />
    </Field>
  );
}

/** Selector de color nativo. */
export function ColorField(p: { label: string; value: string; onInput: (v: string) => void }) {
  return (
    <Field label={p.label}>
      <input type="color" class="pl-stamp-color" value={p.value} onInput={(e) => p.onInput(e.currentTarget.value)} />
    </Field>
  );
}

/** Elegir una imagen del disco y mostrar su nombre. */
export function ImageField(p: { path: string | null; onPick: (path: string) => void }) {
  const pick = async () => {
    const f = await api.pickImageFile(st("stamp.image.title"));
    if (f) p.onPick(f);
  };
  return (
    <Field label={st("stamp.image")}>
      <div class="pl-stamp-image">
        <Button variant="normal" onClick={() => void pick()}>{st("stamp.image.pick")}</Button>
        <span title={p.path ?? undefined}>{p.path ? p.path.split(/[\\/]/).pop() : st("stamp.image.none")}</span>
      </div>
    </Field>
  );
}

/** Lanza `stamp_pdf` sobre el documento activo y sigue el trabajo en el centro de tareas. */
export function useStampRun(jobKey: string, op: () => unknown, pages: () => string, onClose: () => void) {
  const [error, setError] = createSignal<string | null>(null);
  const run = async () => {
    const doc = activeDoc();
    if (!doc) return;
    try {
      const id = await invoke<number>("stamp_pdf", { path: doc.path, pages: pagesOrAll(pages()), op: op() });
      trackJob(id, { title: st(jobKey, { name: doc.title }), retry: () => void run() });
      toast({ tone: "info", text: st("stamp.started") });
      onClose();
    } catch (e) {
      setError(isApiError(e) ? e.message : String(e));
    }
  };
  return { error, run };
}

export function ErrorNotice(p: { error: string | null }) {
  return <Show when={p.error}><Notice tone="danger" inline>{p.error}</Notice></Show>;
}
