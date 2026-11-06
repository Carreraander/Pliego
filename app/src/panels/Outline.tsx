import { createResource, createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { isApiError } from "../api";
import { t } from "../i18n";
import { currentPageIndex, type OpenDoc } from "../state/documents";
import { goToPage } from "../state/navigation";
import { requestOpenUrl } from "../viewer/links/openUrl";
import { withPassword } from "../state/passwords";
import { IconButton, TextInput, Toolbar, ToolbarSeparator, toast } from "../ui";
import {
  addNode, deleteNode, indentNode, moveNode, nodeAt, outdentNode, renameNode, samePath,
  type EditResult, type NodePath, type OutlineNode,
} from "./outlineEdit";

/** Marcadores: ir a cada uno y editarlos (añadir, renombrar, mover, sangrar, borrar) sobre el propio PDF, con copia
 *  previa y deshacer compartido con los demás modos que editan el archivo. */
export function Outline(props: { doc: OpenDoc }) {
  const [items, { mutate }] = createResource(
    () => ({ path: props.doc.path, revision: props.doc.revision }),
    ({ path }) => invoke<OutlineNode[]>("outline_tree", withPassword({ path })).catch(() => [] as OutlineNode[]),
  );
  const [selected, setSelected] = createSignal<NodePath | null>(null);
  const [editing, setEditing] = createSignal<NodePath | null>(null);
  const [undoDepth, setUndoDepth] = createSignal(0);
  const [busy, setBusy] = createSignal(false);

  const apply = async (r: EditResult | null) => {
    if (!r || busy()) return;
    const previous = items() ?? [];
    mutate(r.nodes);
    setSelected(r.path);
    setBusy(true);
    try {
      setUndoDepth(await invoke<number>("outline_save", withPassword({
        docId: props.doc.id, path: props.doc.path, nodes: r.nodes, suffix: t("outline.backup"),
      })));
    } catch (e) {
      mutate(previous);
      toast({ tone: "danger", text: `${t("outline.error")}: ${isApiError(e) ? e.message : String(e)}` });
    } finally {
      setBusy(false);
    }
  };
  const undo = async () => {
    setUndoDepth(await invoke<number>("annot_undo", { docId: props.doc.id, path: props.doc.path }).catch(() => 0));
  };
  const sel = () => selected();
  /** Va al destino; la altura del PDF (origen abajo) se pasa a distancia desde arriba de la página. Un marcador que
   *  abre una web pide confirmación antes, como los enlaces del documento. */
  const go = (item: OutlineNode) => {
    if (item.page === null) { if (item.uri) requestOpenUrl(item.uri); return; }
    const h = props.doc.sizes[item.page]?.height;
    goToPage(item.page, item.top != null && h ? Math.max(0, h - item.top) : undefined);
  };
  const list = () => items() ?? [];
  const add = () => {
    const page = currentPageIndex();
    const r = addNode(list(), sel(), t("outline.newTitle", { page: page + 1 }), page);
    void apply(r).then(() => setEditing(r.path));
  };
  const onKey = (e: KeyboardEvent, path: NodePath) => {
    if (editing()) return;
    if (e.key === "F2") { e.preventDefault(); setEditing(path); }
    else if (e.key === "Delete") { e.preventDefault(); void apply(deleteNode(list(), path)); }
    else if (e.altKey && e.key === "ArrowUp") { e.preventDefault(); void apply(moveNode(list(), path, -1)); }
    else if (e.altKey && e.key === "ArrowDown") { e.preventDefault(); void apply(moveNode(list(), path, 1)); }
    else if (e.altKey && e.key === "ArrowRight") { e.preventDefault(); void apply(indentNode(list(), path)); }
    else if (e.altKey && e.key === "ArrowLeft") { e.preventDefault(); void apply(outdentNode(list(), path)); }
  };

  function Node(p: { item: OutlineNode; path: NodePath }) {
    const isSel = () => samePath(selected(), p.path);
    const commit = (value: string) => {
      setEditing(null);
      if (value.trim() && value.trim() !== p.item.title) void apply(renameNode(list(), p.path, value));
    };
    return (
      <li role="treeitem" aria-selected={isSel()} aria-expanded={p.item.children.length ? true : undefined}>
        <Show when={samePath(editing(), p.path)} fallback={
          <button class="outline-item" classList={{ "outline-item--selected": isSel() }} data-path={p.path.join(".")}
            title={p.item.uri ?? undefined}
            onClick={() => { setSelected(p.path); go(p.item); }}
            onDblClick={() => setEditing(p.path)} onKeyDown={(e) => onKey(e, p.path)}>
            {p.item.title}
          </button>
        }>
          <TextInput ref={(el: HTMLInputElement) => requestAnimationFrame(() => { el.focus(); el.select(); })} value={p.item.title} aria-label={t("outline.rename")}
            onKeyDown={(e) => {
              if (e.key === "Enter") { e.preventDefault(); commit(e.currentTarget.value); }
              if (e.key === "Escape") { e.preventDefault(); setEditing(null); }
            }}
            onBlur={(e) => commit(e.currentTarget.value)} />
        </Show>
        <Show when={p.item.children.length}>
          <ul role="group"><For each={p.item.children}>{(c, i) => <Node item={c} path={[...p.path, i()]} />}</For></ul>
        </Show>
      </li>
    );
  }

  const can = (f: (nodes: OutlineNode[], path: NodePath) => EditResult | null) => {
    const s = sel();
    return !!s && !busy() && !!nodeAt(list(), s) && f(list(), s) !== null;
  };
  const run = (f: (nodes: OutlineNode[], path: NodePath) => EditResult | null) => { const s = sel(); if (s) void apply(f(list(), s)); };

  return (
    <div class="outline" data-testid="outline">
      <Toolbar label={t("panel.outline")} class="outline-toolbar">
        <IconButton icon="plus" size="sm" label={t("outline.add")} disabled={busy()} onClick={add} />
        <IconButton icon="pencil" size="sm" label={t("outline.rename")} disabled={!sel() || busy()} onClick={() => setEditing(sel())} />
        <IconButton icon="chevron-up" size="sm" label={t("outline.up")} disabled={!can((n, s) => moveNode(n, s, -1))} onClick={() => run((n, s) => moveNode(n, s, -1))} />
        <IconButton icon="chevron-down" size="sm" label={t("outline.down")} disabled={!can((n, s) => moveNode(n, s, 1))} onClick={() => run((n, s) => moveNode(n, s, 1))} />
        <IconButton icon="chevron-right" size="sm" label={t("outline.indent")} disabled={!can(indentNode)} onClick={() => run(indentNode)} />
        <IconButton icon="chevron-left" size="sm" label={t("outline.outdent")} disabled={!can(outdentNode)} onClick={() => run(outdentNode)} />
        <IconButton icon="trash" size="sm" variant="danger" label={t("outline.delete")} disabled={!can(deleteNode)} onClick={() => run(deleteNode)} />
        <ToolbarSeparator />
        <IconButton icon="undo" size="sm" label={t("outline.undo")} disabled={undoDepth() === 0 || busy()} onClick={() => void undo()} />
      </Toolbar>
      <Show when={list().length} fallback={<p class="muted">{t("panel.noOutline")}</p>}>
        <ul role="tree" aria-label={t("panel.outline")}><For each={list()}>{(i, k) => <Node item={i} path={[k()]} />}</For></ul>
      </Show>
    </div>
  );
}
