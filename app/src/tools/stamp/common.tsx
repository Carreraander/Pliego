import "./stamp.css";
import { api } from "../../api";
import { activeDoc } from "../../state/documents";
import { Button, Field, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
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

/** Lanza `stamp_pdf` sobre el documento activo con `runToolJob` (centro de tareas, «Reintentar» y toast). */
export function useStampRun(jobKey: string, op: () => unknown, pages: () => string, onClose: () => void) {
  const run = async () => {
    const doc = activeDoc();
    if (!doc) return;
    const id = await runToolJob({
      command: "stamp_pdf",
      args: { path: doc.path, pages: pagesOrAll(pages()), op: op() },
      title: st(jobKey, { name: fileName(doc.path) }),
      started: st("stamp.started"),
    });
    if (id !== null) onClose();
  };
  return { run };
}
