import "./sign.css";
import { createSignal, Show } from "solid-js";
import { locale } from "../../i18n";
import { currentPageIndex, activeDoc } from "../../state/documents";
import { Checkbox, Field, Notice, Select, TaskPanel, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
import { KeyChooser } from "./KeyChooser";
import { emptyKey, keySource, missing, needsTimestamp, signOptions, type Corner, type Level, type SignForm } from "./request";
import { st } from "./text";

export default function SignDigitalPanel(p: { onClose: () => void }) {
  const [form, setForm] = createSignal<SignForm>({
    key: emptyKey(), level: "b-b", tsa: "", visible: false, page: currentPageIndex() + 1, corner: "bottom-right",
    certify: 0, reason: "", location: "", contact: "",
  });
  const [tried, setTried] = createSignal(false);
  const set = (patch: Partial<SignForm>) => setForm((f) => ({ ...f, ...patch }));
  const problem = () => missing(form());

  const run = async () => {
    const doc = activeDoc();
    if (!doc) return;
    setTried(true);
    if (problem()) return;
    const f = form();
    const id = await runToolJob({
      command: "sign_pdf", args: { path: doc.path, key: keySource(f.key), options: signOptions(f, doc.sizes[f.page - 1], locale()), suffix: st("sign.suffix") },
      title: st("sign.job", { name: fileName(doc.path) }), started: st("sign.started"),
    });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={st("sign.title")} onClose={p.onClose}
      primary={{ label: st("sign.run"), icon: "sign-digital", onClick: () => void run() }}
      estimate={st("sign.hint")}
      notice={<Show when={tried() && problem() ? st(`sign.missing.${problem()}`) : null}>{(m) => <Notice tone="danger" inline>{m()}</Notice>}</Show>}
      advanced={
        <>
          <Field label={st("sign.certify")}>
            <Select
              value={String(form().certify)} onChange={(e) => set({ certify: Number(e.currentTarget.value) as SignForm["certify"] })}
              options={["0", "1", "2", "3"].map((v) => ({ value: v, label: st(`sign.certify.${v}`) }))}
            />
          </Field>
          <Field label={st("sign.reason")}><TextInput value={form().reason} onInput={(e) => set({ reason: e.currentTarget.value })} /></Field>
          <Field label={st("sign.location")}><TextInput value={form().location} onInput={(e) => set({ location: e.currentTarget.value })} /></Field>
          <Field label={st("sign.contact")}><TextInput value={form().contact} onInput={(e) => set({ contact: e.currentTarget.value })} /></Field>
        </>
      }
    >
      <KeyChooser value={form().key} onChange={(key) => set({ key })} />
      <Field label={st("sign.level")}>
        <Select
          value={form().level} onChange={(e) => set({ level: e.currentTarget.value as Level })}
          options={[
            { value: "b-b", label: st("sign.level.bb") }, { value: "b-t", label: st("sign.level.bt") },
            { value: "b-lt", label: st("sign.level.blt") }, { value: "b-lta", label: st("sign.level.blta") },
          ]}
        />
      </Field>
      <Show when={needsTimestamp(form().level)}>
        <Field label={st("sign.tsa")} hint={st("sign.tsa.hint")}>
          <TextInput type="url" placeholder="http://…" value={form().tsa} onInput={(e) => set({ tsa: e.currentTarget.value })} />
        </Field>
      </Show>
      <Checkbox label={st("sign.visible")} checked={form().visible} onChange={(e) => set({ visible: e.currentTarget.checked })} />
      <Show when={form().visible}>
        <Field label={st("sign.page")}>
          <TextInput type="number" min="1" max={activeDoc()?.sizes.length ?? 1} value={form().page} onInput={(e) => set({ page: Number(e.currentTarget.value) })} />
        </Field>
        <Field label={st("sign.corner")}>
          <Select
            value={form().corner} onChange={(e) => set({ corner: e.currentTarget.value as Corner })}
            options={[
              { value: "bottom-left", label: st("sign.corner.bl") }, { value: "bottom-right", label: st("sign.corner.br") },
              { value: "top-left", label: st("sign.corner.tl") }, { value: "top-right", label: st("sign.corner.tr") },
            ]}
          />
        </Field>
      </Show>
    </TaskPanel>
  );
}
