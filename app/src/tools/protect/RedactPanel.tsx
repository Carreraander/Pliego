import { createSignal, For, onCleanup, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { Button, Checkbox, Field, Notice, RadioGroup, Radio, TaskPanel, TextInput } from "../../ui";
import { invoke } from "@tauri-apps/api/core";
import { activeDoc } from "../../state/documents";
import { errorText, runToolJob } from "../run";
import { baseName, buildRedactRequest, patternNames, REDACT_PATTERNS, type RedactForm } from "./request";
import { areasOf, clearAreas, marking, matchesOf, setMarking, setMatches, type FoundMatch } from "./redactState";
import { pt } from "./strings";
import { withPassword } from "../../state/passwords";

export default function RedactPanel(p: { onClose?: () => void }) {
  const [form, setForm] = createStore<RedactForm>({
    patterns: [], regex: "", useRegex: false, areas: [], fill: "Black", images: "Pixels", removeLineArt: true, current: "",
  });
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  onCleanup(() => setMarking(false));
  const doc = () => activeDoc();
  const hasAreas = () => (doc() ? areasOf(doc()!.id).length : 0);
  const matches = () => (doc() ? matchesOf(doc()!.id) : []);
  const pages = () => new Set(matches().map((m) => m.page)).size;

  const toggle = (k: (typeof REDACT_PATTERNS)[number], on: boolean) =>
    setForm("patterns", on ? [...form.patterns, k] : form.patterns.filter((x) => x !== k));

  const find = async () => {
    const d = doc();
    if (!d) return;
    const names = patternNames(form);
    if (form.useRegex && !form.regex.trim()) { setError(pt("redact.err.regex")); return; }
    setError(null); setBusy(true);
    try {
      setMatches(d.id, await invoke<FoundMatch[]>("redact_find", withPassword({ path: d.path, password: form.current || null, patterns: names })));
    } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };

  const apply = async () => {
    const d = doc();
    if (!d) return;
    const built = buildRedactRequest(d.path, { ...form, patterns: [...form.patterns], areas: [...areasOf(d.id)] });
    if (!built.ok) { setError(pt(built.error)); return; }
    setError(null);
    const id = await runToolJob({
      command: "redact_apply", args: { request: { ...built.request, suffix: pt("redact.suffix") } },
      title: pt("redact.job", { name: baseName(d.path) }), started: pt("redact.started"),
    });
    if (id !== null) { setMarking(false); setMatches(d.id, []); clearAreas(d.id); p.onClose?.(); }
  };

  return (
    <TaskPanel
      title={pt("redact.panel")} onClose={() => { setMarking(false); p.onClose?.(); }}
      primary={{ label: pt("redact.apply"), onClick: () => void apply(), variant: "danger" }}
      notice={<Show when={error()}><Notice tone="danger">{error()}</Notice></Show>}
      estimate={pt("protect.copyNote")}
      advanced={
        <>
          <RadioGroup legend={pt("redact.fill")} value={form.fill} onChange={(v) => setForm("fill", v as RedactForm["fill"])}>
            <Radio value="Black" label={pt("redact.fill.black")} /><Radio value="None" label={pt("redact.fill.none")} />
          </RadioGroup>
          <RadioGroup legend={pt("redact.images")} value={form.images} onChange={(v) => setForm("images", v as RedactForm["images"])}>
            <Radio value="Pixels" label={pt("redact.images.pixels")} /><Radio value="Remove" label={pt("redact.images.remove")} /><Radio value="Keep" label={pt("redact.images.keep")} />
          </RadioGroup>
          <Checkbox label={pt("redact.lineart")} checked={form.removeLineArt} onChange={(e) => setForm("removeLineArt", e.currentTarget.checked)} />
          <Field label={pt("protect.current")} hint={pt("protect.current.hint")}>
            <TextInput type="password" autocomplete="off" value={form.current} onInput={(e) => setForm("current", e.currentTarget.value)} />
          </Field>
        </>
      }
    >
      <p class="pl-muted">{pt("redact.intro")}</p>
      <Button variant={marking() ? "primary" : "normal"} icon="redact" onClick={() => setMarking(!marking())}>{marking() ? pt("redact.marking") : pt("redact.mark")}</Button>
      <p class="pl-muted">{pt("redact.areas", { n: hasAreas() })}</p>
      <Show when={hasAreas() > 0}><Button variant="ghost" onClick={() => doc() && clearAreas(doc()!.id)}>{pt("redact.areas.clear")}</Button></Show>

      <fieldset class="pl-radiogroup">
        <legend>{pt("redact.patterns")}</legend>
        <For each={REDACT_PATTERNS}>{(k) => <Checkbox label={pt(`pattern.${k}`)} checked={form.patterns.includes(k)} onChange={(e) => toggle(k, e.currentTarget.checked)} />}</For>
        <Checkbox label={pt("pattern.regex")} checked={form.useRegex} onChange={(e) => setForm("useRegex", e.currentTarget.checked)} />
        <Show when={form.useRegex}>
          <Field label={pt("redact.regex")} hint={pt("redact.regex.hint")}>
            <TextInput value={form.regex} onInput={(e) => setForm("regex", e.currentTarget.value)} />
          </Field>
        </Show>
      </fieldset>
      <Button icon="search" disabled={busy() || patternNames(form).length === 0} onClick={() => void find()}>{busy() ? pt("redact.finding") : pt("redact.find")}</Button>
      <Show when={matches().length > 0}>
        <Notice tone="warn">{pt("redact.found", { n: matches().length, pages: pages() })}</Notice>
        <ul class="pl-muted">
          <For each={matches().slice(0, 30)}>{(m) => <li>{pt("redact.page", { n: m.page + 1 })} · {m.pattern}: {m.text}</li>}</For>
        </ul>
      </Show>
    </TaskPanel>
  );
}
