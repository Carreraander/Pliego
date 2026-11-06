import { createSignal } from "solid-js";
import { activeDoc } from "../../state/documents";
import { Field, TaskPanel, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
import { cropArgs } from "./request";
import { ot } from "./text";

/** Recortar páginas: quita márgenes a la caja visible de las páginas (lo de fuera deja de verse e imprimirse); guarda una copia. */
export default function CropPanel(p: { onClose: () => void }) {
  const [pages, setPages] = createSignal("");
  const [m, setM] = createSignal({ left: "10", top: "10", right: "10", bottom: "10" });
  const total = () => activeDoc()?.sizes.length ?? 0;
  const args = () => cropArgs({ path: activeDoc()?.path ?? "", total: total(), pages: pages(), ...m(), suffix: ot("organize.crop.suffix") });
  const set = (k: "left" | "top" | "right" | "bottom") => (e: { currentTarget: HTMLInputElement }) => setM({ ...m(), [k]: e.currentTarget.value });

  const run = async () => {
    const a = args();
    const d = activeDoc();
    if (!a || !d) return;
    const id = await runToolJob({ command: "crop_pages", args: a, title: ot("organize.crop.job", { name: fileName(d.path) }), started: ot("organize.crop.started") });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={ot("organize.crop.title")} onClose={p.onClose}
      primary={{ label: ot("organize.crop.run"), icon: "crop", disabled: args() === null, onClick: () => void run() }}
      estimate={ot("organize.crop.hint")}
    >
      <Field label={ot("organize.crop.top")}><TextInput inputmode="decimal" value={m().top} onInput={set("top")} /></Field>
      <Field label={ot("organize.crop.bottom")}><TextInput inputmode="decimal" value={m().bottom} onInput={set("bottom")} /></Field>
      <Field label={ot("organize.crop.left")}><TextInput inputmode="decimal" value={m().left} onInput={set("left")} /></Field>
      <Field label={ot("organize.crop.right")}><TextInput inputmode="decimal" value={m().right} onInput={set("right")} /></Field>
      <Field label={ot("organize.crop.pages")} hint={ot("organize.crop.pagesHint")}><TextInput value={pages()} onInput={(e) => setPages(e.currentTarget.value)} /></Field>
    </TaskPanel>
  );
}
