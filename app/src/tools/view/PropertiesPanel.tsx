import { createEffect, createSignal, For, on, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { api } from "../../api";
import { activeDoc } from "../../state/documents";
import { withPassword } from "../../state/passwords";
import { Field, Notice, TaskPanel, TextInput, toast } from "../../ui";
import { errorText } from "../run";
import { EDITABLE, infoEdit, infoForm, propertyRows, type FileFacts, type InfoForm, type PropertyRow } from "./properties";
import { vt } from "./strings";

const keyText: Record<string, string> = { title: "view.props.title.row" };
const label = (key: string) => vt(keyText[key] ?? `view.props.${key}`);

const EMPTY: InfoForm = { title: "", author: "", subject: "", keywords: "" };

/** Propiedades del documento: título, autor, asunto y palabras clave se pueden cambiar (en el propio PDF, con copia
 *  previa y deshacer); el resto (páginas, tamaño, ubicación…) es de solo lectura. */
export default function PropertiesPanel(p: { onClose: () => void }) {
  const [rows, setRows] = createSignal<PropertyRow[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [before, setBefore] = createSignal<InfoForm>(EMPTY);
  const [form, setForm] = createSignal<InfoForm>(EMPTY);
  const [busy, setBusy] = createSignal(false);
  const [undoDepth, setUndoDepth] = createSignal(0);

  createEffect(on(() => [activeDoc()?.id, activeDoc()?.revision] as const, () => {
    const doc = activeDoc();
    setRows([]); setError(null);
    if (!doc) return;
    void (async () => {
      try {
        const [info, file] = await Promise.all([
          api.documentInfo(doc.id),
          invoke<FileFacts>("file_facts", { path: doc.path }).catch(() => null),
        ]);
        const date = (secs: number) => new Date(secs * 1000).toLocaleString();
        setRows(propertyRows(info, file, doc.path, doc.sizes[0] ?? null, date).filter((r) => !(EDITABLE as readonly string[]).includes(r.key)));
        setBefore(infoForm(info));
        setForm(infoForm(info));
      } catch (e) {
        setError(vt("view.props.failed", { error: errorText(e) }));
      }
    })();
  }));

  const edit = () => infoEdit(before(), form());
  const save = async () => {
    const d = activeDoc();
    const e = edit();
    if (!d || !e || busy()) return;
    setBusy(true);
    try {
      setUndoDepth(await invoke<number>("properties_save", withPassword({ docId: d.id, path: d.path, edit: e, suffix: vt("view.props.backup") })));
      setBefore({ ...form() });
      toast({ tone: "ok", text: vt("view.props.saved") });
    } catch (err) {
      toast({ tone: "danger", text: vt("view.props.saveFailed", { error: errorText(err) }) });
    } finally {
      setBusy(false);
    }
  };
  const undo = async () => {
    const d = activeDoc();
    if (d) setUndoDepth(await invoke<number>("annot_undo", { docId: d.id, path: d.path }).catch(() => 0));
  };
  const show = () => { const d = activeDoc(); if (d) void invoke("reveal_path", { path: d.path }); };

  return (
    <TaskPanel
      title={vt("view.props.title")} onClose={p.onClose}
      primary={{ label: vt("view.props.save"), icon: "check", disabled: !activeDoc() || !edit() || busy(), onClick: () => void save() }}
      secondary={[
        ...(undoDepth() > 0 ? [{ label: vt("view.props.undo"), icon: "undo", onClick: () => void undo() }] : []),
        { label: vt("view.props.show"), icon: "folder-search", disabled: !activeDoc(), onClick: show },
      ]}
      estimate={vt("view.props.hint")}
      notice={<Show when={!activeDoc() || error()}><Notice tone={error() ? "danger" : "warn"} inline>{error() ?? vt("view.props.noDoc")}</Notice></Show>}
    >
      <Show when={activeDoc() && !error()}>
        <For each={EDITABLE}>
          {(k) => (
            <Field label={label(k)}>
              <TextInput value={form()[k]} onInput={(e) => setForm({ ...form(), [k]: e.currentTarget.value })}
                onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); void save(); } }} />
            </Field>
          )}
        </For>
      </Show>
      <dl>
        <For each={rows()}>{(r) => (<><dt class="pl-field__label">{label(r.key)}</dt><dd>{r.value}</dd></>)}</For>
      </dl>
    </TaskPanel>
  );
}
