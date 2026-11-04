import { createEffect, createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { api } from "../../api";
import { activeDoc } from "../../state/documents";
import { Notice, TaskPanel } from "../../ui";
import { errorText } from "../run";
import { propertyRows, type FileFacts, type PropertyRow } from "./properties";
import { vt } from "./strings";

const keyText: Record<string, string> = { title: "view.props.title.row" };
const label = (key: string) => vt(keyText[key] ?? `view.props.${key}`);

/** Propiedades del documento (solo lectura): metadatos, páginas, tamaño y ubicación del archivo. */
export default function PropertiesPanel(p: { onClose: () => void }) {
  const [rows, setRows] = createSignal<PropertyRow[]>([]);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
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
        setRows(propertyRows(info, file, doc.path, doc.sizes[0] ?? null, date));
      } catch (e) {
        setError(vt("view.props.failed", { error: errorText(e) }));
      }
    })();
  });

  const show = () => { const d = activeDoc(); if (d) void invoke("reveal_path", { path: d.path }); };

  return (
    <TaskPanel
      title={vt("view.props.title")} onClose={p.onClose}
      primary={{ label: vt("view.props.show"), icon: "folder-search", disabled: !activeDoc(), onClick: show }}
      notice={<Show when={!activeDoc() || error()}><Notice tone={error() ? "danger" : "warn"} inline>{error() ?? vt("view.props.noDoc")}</Notice></Show>}
    >
      <dl>
        <For each={rows()}>{(r) => (<><dt class="pl-field__label">{label(r.key)}</dt><dd>{r.value}</dd></>)}</For>
      </dl>
    </TaskPanel>
  );
}
