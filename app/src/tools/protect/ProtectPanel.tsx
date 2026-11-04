import { createSignal, For, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { Checkbox, Field, Notice, Select, TaskPanel, TextInput } from "../../ui";
import { runToolJob } from "../run";
import { activeDoc } from "../../state/documents";
import { ALL_ALLOWED, baseName, buildProtectRequest, PERMISSIONS, type Permissions } from "./request";
import { pt } from "./strings";

/** Permisos que se ven siempre; accesibilidad y alta calidad van en «avanzado». */
const MAIN = PERMISSIONS.filter((k) => k !== "accessibility" && k !== "print_high_quality");

export default function ProtectPanel(p: { onClose?: () => void }) {
  const [userPassword, setUser] = createSignal("");
  const [repeat, setRepeat] = createSignal("");
  const [ownerPassword, setOwner] = createSignal("");
  const [current, setCurrent] = createSignal("");
  const [cipher, setCipher] = createSignal<"Aes256" | "Aes128">("Aes256");
  const [perms, setPerms] = createStore<Permissions>({ ...ALL_ALLOWED });
  const [error, setError] = createSignal<string | null>(null);

  const apply = async () => {
    const doc = activeDoc();
    if (!doc) return;
    const built = buildProtectRequest(doc.path, {
      userPassword: userPassword(), repeat: repeat(), ownerPassword: ownerPassword(), current: current(), cipher: cipher(), permissions: { ...perms },
    });
    if (!built.ok) { setError(pt(built.error)); return; }
    setError(null);
    const id = await runToolJob({
      command: "protect_encrypt", args: { request: built.request },
      title: pt("protect.job", { name: baseName(doc.path) }), started: pt("protect.started"),
    });
    if (id !== null) p.onClose?.();
  };

  const remove = async () => {
    const doc = activeDoc();
    if (!doc) return;
    if (!current()) { setError(pt("protect.err.current")); return; }
    setError(null);
    const id = await runToolJob({
      command: "protect_remove", args: { path: doc.path, password: current() },
      title: pt("protect.job.remove", { name: baseName(doc.path) }), started: pt("protect.started"),
    });
    if (id !== null) p.onClose?.();
  };

  return (
    <TaskPanel
      title={pt("protect.panel")} onClose={p.onClose}
      primary={{ label: pt("protect.save"), onClick: () => void apply() }}
      secondary={[{ label: pt("protect.remove"), onClick: () => void remove(), variant: "ghost" }]}
      notice={<Show when={error()}><Notice tone="danger">{error()}</Notice></Show>}
      estimate={pt("protect.copyNote")}
      advanced={
        <>
          <Field label={pt("protect.cipher")}>
            <Select value={cipher()} onChange={(e) => setCipher(e.currentTarget.value as "Aes256" | "Aes128")}
              options={[{ value: "Aes256", label: pt("protect.cipher.256") }, { value: "Aes128", label: pt("protect.cipher.128") }]} />
          </Field>
          <Checkbox label={pt("perm.accessibility")} checked={perms.accessibility} onChange={(e) => setPerms("accessibility", e.currentTarget.checked)} />
          <Checkbox label={pt("perm.print_high_quality")} checked={perms.print_high_quality} onChange={(e) => setPerms("print_high_quality", e.currentTarget.checked)} />
        </>
      }
    >
      <Field label={pt("protect.open")} hint={pt("protect.open.hint")}>
        <TextInput type="password" autocomplete="new-password" value={userPassword()} onInput={(e) => setUser(e.currentTarget.value)} />
      </Field>
      <Show when={userPassword()}>
        <Field label={pt("protect.repeat")}>
          <TextInput type="password" autocomplete="new-password" value={repeat()} onInput={(e) => setRepeat(e.currentTarget.value)} />
        </Field>
      </Show>
      <Field label={pt("protect.owner")} hint={pt("protect.owner.hint")}>
        <TextInput type="password" autocomplete="new-password" value={ownerPassword()} onInput={(e) => setOwner(e.currentTarget.value)} />
      </Field>
      <fieldset class="pl-radiogroup">
        <legend>{pt("protect.allow")}</legend>
        <For each={MAIN}>
          {(k) => <Checkbox label={pt(`perm.${k}`)} checked={perms[k]} onChange={(e) => setPerms(k, e.currentTarget.checked)} />}
        </For>
      </fieldset>
      <Field label={pt("protect.current")} hint={pt("protect.current.hint")}>
        <TextInput type="password" autocomplete="off" value={current()} onInput={(e) => setCurrent(e.currentTarget.value)} />
      </Field>
    </TaskPanel>
  );
}
