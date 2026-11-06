import { withPassword } from "../../state/passwords";
import { createEffect, createSignal, For, on, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { api } from "../../api";
import { activeDoc } from "../../state/documents";
import { Button, Notice, TaskPanel, toast } from "../../ui";
import { sizeLabel, type PortfolioEntry, type PortfolioListing } from "../batch/attachments";
import { errorText, fileName, runToolJob } from "../run";
import { vt } from "./strings";

/** Archivos adjuntos del documento: lista, extracción (todos o uno) y añadir o quitar sobre el propio PDF, con copia
 *  previa y deshacer compartido con los demás modos que editan el archivo. */
export default function AttachmentsPanel(p: { onClose: () => void }) {
  const [entries, setEntries] = createSignal<PortfolioEntry[] | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [undoDepth, setUndoDepth] = createSignal(0);

  createEffect(on(() => [activeDoc()?.path, activeDoc()?.revision] as const, ([path]) => {
    setError(null);
    if (!path) { setEntries(null); return; }
    invoke<PortfolioListing>("portfolio_list", withPassword({ path }))
      .then((l) => setEntries(l.entries))
      .catch((e) => setError(vt("view.att.failed", { error: errorText(e) })));
  }));

  const extract = async (names: string[]) => {
    const doc = activeDoc();
    if (!doc) return;
    const id = await runToolJob({
      command: "portfolio_extract", args: { path: doc.path, names, suffix: vt("view.att.suffix") },
      title: vt("view.att.job", { name: fileName(doc.path) }), started: vt("view.att.started"),
    });
    if (id !== null) p.onClose();
  };

  /** Cambia el PDF y relee la lista (el visor se recarga solo). */
  const edit = async (command: "attachments_add" | "attachments_remove", args: Record<string, unknown>, done: string) => {
    const doc = activeDoc();
    if (!doc || busy()) return;
    setBusy(true);
    try {
      setUndoDepth(await invoke<number>(command, withPassword({ docId: doc.id, path: doc.path, suffix: vt("view.att.backup"), ...args })));
      const l = await invoke<PortfolioListing>("portfolio_list", withPassword({ path: doc.path }));
      setEntries(l.entries);
      toast({ tone: "ok", text: done });
    } catch (e) {
      toast({ tone: "danger", text: vt("view.att.editFailed", { error: errorText(e) }) });
    } finally {
      setBusy(false);
    }
  };
  const add = async () => {
    const files = await api.pickAnyFiles(vt("view.att.pick"));
    if (files.length) await edit("attachments_add", { files }, vt("view.att.added", { n: files.length }));
  };
  const remove = (e: PortfolioEntry) => edit("attachments_remove", { names: [e.name] }, vt("view.att.removed", { name: e.filename }));
  const undo = async () => {
    const doc = activeDoc();
    if (!doc) return;
    setUndoDepth(await invoke<number>("annot_undo", { docId: doc.id, path: doc.path }).catch(() => 0));
    const l = await invoke<PortfolioListing>("portfolio_list", withPassword({ path: doc.path })).catch(() => null);
    if (l) setEntries(l.entries);
  };

  return (
    <TaskPanel
      title={vt("view.att.title")} onClose={p.onClose} estimate={vt("view.att.hint")}
      primary={{ label: vt("view.att.extractAll"), icon: "download", disabled: !entries()?.length || busy(), onClick: () => void extract([]) }}
      secondary={[
        { label: vt("view.att.add"), icon: "plus", disabled: !activeDoc() || busy(), onClick: () => void add() },
        ...(undoDepth() > 0 ? [{ label: vt("view.att.undo"), icon: "undo", disabled: busy(), onClick: () => void undo() }] : []),
      ]}
      notice={<Show when={!activeDoc() || error()}><Notice tone={error() ? "danger" : "warn"} inline>{error() ?? vt("view.att.noDoc")}</Notice></Show>}
    >
      <Show when={entries()}>
        {(list) => (
          <Show when={list().length} fallback={<p>{vt("view.att.none")}</p>}>
            <ul class="pl-attachments">
              <For each={list()}>{(e) => (
                <li>
                  <span>{e.filename} <small>({sizeLabel(e.size)})</small></span>
                  <Button onClick={() => void extract([e.name])}>{vt("view.att.extract")}</Button>
                  <Button variant="danger" disabled={busy()} onClick={() => void remove(e)}>{vt("view.att.remove")}</Button>
                </li>
              )}</For>
            </ul>
          </Show>
        )}
      </Show>
    </TaskPanel>
  );
}
