import { Show } from "solid-js";
import { activeDoc } from "../../state/documents";
import { Button, IconButton, ToolbarSeparator } from "../../ui";
import { fileName, runToolJob } from "../run";
import { deleteSelected, duplicateSelected, isDirty, moveSelected, rotateSelected, selectAll, selectionEdits, toEdits } from "./pageedits";
import { editsOf, resetEdits, updateEdits } from "./store";
import { ot } from "./text";

/** Barra del modo Organizar. Actúa sobre las páginas seleccionadas de la cuadrícula. */
export function OrganizeToolbar() {
  const doc = () => activeDoc();
  const pages = () => doc()?.sizes.length ?? 0;
  const edits = () => editsOf(doc()?.id ?? null, pages());
  const has = () => edits().selected.length > 0;
  const update = (f: Parameters<typeof updateEdits>[2]) => { const d = doc(); if (d) updateEdits(d.id, pages(), f); };

  const save = async (selectionOnly: boolean) => {
    const d = doc();
    if (!d) return;
    const list = selectionOnly ? selectionEdits(edits()) : toEdits(edits());
    const id = await runToolJob({
      command: "organize_pages",
      args: { path: d.path, edits: list, suffix: ot(selectionOnly ? "organize.grid.suffixExtract" : "organize.grid.suffix") },
      title: ot(selectionOnly ? "organize.grid.jobExtract" : "organize.grid.job", { name: fileName(d.path) }),
      started: ot("organize.grid.started"),
    });
    if (id !== null && !selectionOnly) resetEdits(d.id);
  };

  return (
    <>
      <IconButton icon="rotate-ccw" label={ot("organize.grid.rotateLeft")} disabled={!has()} onClick={() => update((e) => rotateSelected(e, -90))} />
      <IconButton icon="rotate-cw" label={ot("organize.grid.rotateRight")} disabled={!has()} onClick={() => update((e) => rotateSelected(e, 90))} />
      <IconButton icon="arrow-left" label={ot("organize.grid.moveLeft")} disabled={!has()} onClick={() => update((e) => moveSelected(e, -1))} />
      <IconButton icon="arrow-right" label={ot("organize.grid.moveRight")} disabled={!has()} onClick={() => update((e) => moveSelected(e, 1))} />
      <IconButton icon="copy" label={ot("organize.grid.duplicate")} disabled={!has()} onClick={() => update(duplicateSelected)} />
      <IconButton icon="trash" variant="danger" label={ot("organize.grid.delete")} disabled={!has() || edits().selected.length >= edits().items.length} onClick={() => update(deleteSelected)} />
      <ToolbarSeparator />
      <Button variant="ghost" onClick={() => update(selectAll)} disabled={!doc()}>{ot("organize.grid.selectAll")}</Button>
      <Button variant="ghost" icon="scissors" onClick={() => void save(true)} disabled={!has()}>{ot("organize.grid.extract")}</Button>
      <ToolbarSeparator />
      <Show when={doc() && isDirty(edits(), pages())}>
        <Button variant="ghost" icon="undo" onClick={() => doc() && resetEdits(doc()!.id)}>{ot("organize.grid.discard")}</Button>
      </Show>
      <Button variant="primary" icon="download" onClick={() => void save(false)} disabled={!doc() || !isDirty(edits(), pages())}>{ot("organize.grid.save")}</Button>
    </>
  );
}
