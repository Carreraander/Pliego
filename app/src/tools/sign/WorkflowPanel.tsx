import "./sign.css";
import { createResource, createSignal, For, Match, Show, Switch } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { trackJob } from "../../jobs";
import { activeDoc } from "../../state/documents";
import { Button, Field, IconButton, Notice, Radio, RadioGroup, TaskPanel, TextInput, toast } from "../../ui";
import { errorMessage } from "./errors";
import { KeyChooser } from "./KeyChooser";
import { emptyKey, keySource, signerSpecs, type KeyChoice, type SignerRow } from "./request";
import { st } from "./text";

type Mode = "prepare" | "sign" | "audit";
interface Pending { index: number; total: number; spec: { name: string }; field_name: string }
interface AuditEntry {
  time: string | null; event?: string; certificate?: string; expected?: string | null; in_order?: boolean; integrity?: boolean; signers?: string[];
}

const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;
const blank = (): SignerRow => ({ name: "", email: "", certificateContains: "" });

export default function WorkflowPanel(p: { onClose: () => void }) {
  const [mode, setMode] = createSignal<Mode>("prepare");
  const [rows, setRows] = createSignal<SignerRow[]>([blank()]);
  const [key, setKey] = createSignal<KeyChoice>(emptyKey());
  const [error, setError] = createSignal<string | null>(null);
  const path = () => activeDoc()?.path ?? "";

  // Turno y registro: solo se piden cuando se muestran (el documento puede no ser un paquete).
  const [turn] = createResource(() => (mode() === "sign" ? path() : null), (src) =>
    invoke<Pending | null>("signflow_next", { path: src }).then((v) => ({ ok: true as const, v }), (e) => ({ ok: false as const, e: errorMessage(e) })));
  const [audit] = createResource(() => (mode() === "audit" ? path() : null), (src) =>
    invoke<AuditEntry[]>("signflow_audit", { path: src }).then((v) => ({ ok: true as const, v }), (e) => ({ ok: false as const, e: errorMessage(e) })));

  const specs = () => signerSpecs(rows());
  const edit = (i: number, patch: Partial<SignerRow>) => setRows((all) => all.map((r, k) => (k === i ? { ...r, ...patch } : r)));
  const move = (i: number, d: -1 | 1) => setRows((all) => {
    const j = i + d;
    if (j < 0 || j >= all.length) return all;
    const next = [...all];
    [next[i], next[j]] = [next[j]!, next[i]!];
    return next;
  });

  const launch = async (command: string, args: Record<string, unknown>, title: string, started: string) => {
    try {
      const id = await invoke<number>(command, args);
      trackJob(id, { title, retry: () => void launch(command, args, title, started) });
      toast({ tone: "info", text: started });
      p.onClose();
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const prepare = () => launch("signflow_prepare", { path: path(), signers: specs() }, st("flow.prepare.job", { name: fileName(path()) }), st("flow.prepare.started"));
  const signTurn = () => {
    const src = keySource(key());
    if (!src) { setError(st("sign.missing.key")); return; }
    return launch("signflow_sign", { path: path(), key: src, options: {} }, st("flow.sign.job", { name: pending()?.spec.name ?? "" }), st("flow.sign.started"));
  };

  const pending = () => { const t = turn(); return t && t.ok ? t.v : null; };
  const log = (): AuditEntry[] => { const a = audit(); return a && a.ok ? a.v : []; };
  const primary = () => {
    switch (mode()) {
      case "prepare": return { label: st("flow.prepare"), icon: "users", disabled: specs().length < 1, onClick: () => void prepare() };
      case "sign": return { label: st("flow.sign"), icon: "sign-digital", disabled: !pending(), onClick: () => void signTurn() };
      case "audit": return { label: st("flow.close"), onClick: p.onClose };
    }
  };

  return (
    <TaskPanel
      title={st("flow.title")} onClose={p.onClose} primary={primary()}
      estimate={mode() === "prepare" ? (specs().length < 1 ? st("flow.prepare.min") : st("flow.prepare.hint")) : undefined}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <RadioGroup legend={st("flow.mode")} value={mode()} onChange={(v) => { setError(null); setMode(v as Mode); }}>
        <Radio value="prepare" label={st("flow.mode.prepare")} />
        <Radio value="sign" label={st("flow.mode.sign")} />
        <Radio value="audit" label={st("flow.mode.audit")} />
      </RadioGroup>
      <Switch>
        <Match when={mode() === "prepare"}>
          <p class="pl-field__label">{st("flow.signers")}</p>
          <ol class="pl-sign-signers">
            <For each={rows()}>{(r, i) => (
              <li>
                <Field label={`${i() + 1}. ${st("flow.name")}`}><TextInput value={r.name} onInput={(e) => edit(i(), { name: e.currentTarget.value })} /></Field>
                <Field label={st("flow.email")}><TextInput type="email" value={r.email} onInput={(e) => edit(i(), { email: e.currentTarget.value })} /></Field>
                <Field label={st("flow.cert")}><TextInput value={r.certificateContains} onInput={(e) => edit(i(), { certificateContains: e.currentTarget.value })} /></Field>
                <div class="pl-sign-actions">
                  <IconButton icon="chevron-up" label={st("flow.up")} size="sm" disabled={i() === 0} onClick={() => move(i(), -1)} />
                  <IconButton icon="chevron-down" label={st("flow.down")} size="sm" disabled={i() === rows().length - 1} onClick={() => move(i(), 1)} />
                  <IconButton icon="x" label={st("flow.remove")} size="sm" disabled={rows().length === 1} onClick={() => setRows((all) => all.filter((_, k) => k !== i()))} />
                </div>
              </li>
            )}</For>
          </ol>
          <Button icon="plus" onClick={() => setRows((all) => [...all, blank()])}>{st("flow.add")}</Button>
        </Match>
        <Match when={mode() === "sign"}>
          <Show when={!turn.loading} fallback={<p class="pl-muted">{st("validate.loading")}</p>}>
            <Show when={turn()?.ok === false}><Notice tone="warn" inline>{st("flow.notPackage")}</Notice></Show>
            <Show when={turn()?.ok && !pending()}><Notice tone="ok" inline>{st("flow.done")}</Notice></Show>
            <Show when={pending()}>{(n) => <Notice tone="info" inline>{st("flow.next", { name: n().spec.name, n: n().index + 1, total: n().total })}</Notice>}</Show>
          </Show>
          <Show when={pending()}><KeyChooser value={key()} onChange={setKey} /></Show>
        </Match>
        <Match when={mode() === "audit"}>
          <Show when={!audit.loading} fallback={<p class="pl-muted">{st("validate.loading")}</p>}>
            <Show when={audit()?.ok === false}><Notice tone="warn" inline>{st("flow.notPackage")}</Notice></Show>
            <Show when={audit()?.ok && log().length === 0}><p class="pl-muted">{st("flow.audit.empty")}</p></Show>
            <ul class="pl-sign-list">
              <For each={log()}>{(e) => (
                <li class="pl-sign-card">
                  <Show when={e.signers}>{(s) => <span>{st("flow.audit.prepared", { names: s().join(", ") })}</span>}</Show>
                  <Show when={e.certificate}>
                    <span>{st("flow.audit.signed", { name: e.certificate ?? "" })}{e.in_order === false ? ` · ${st("flow.audit.outOfOrder")}` : ""}{e.integrity === false ? ` · ${st("flow.audit.bad")}` : ""}</span>
                  </Show>
                  <span class="pl-muted">{e.time ?? ""}</span>
                </li>
              )}</For>
            </ul>
          </Show>
        </Match>
      </Switch>
    </TaskPanel>
  );
}
