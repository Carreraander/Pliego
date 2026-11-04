import { activeDoc } from "../../state/documents";
import { IconButton, ToggleGroup, ToolbarSeparator } from "../../ui";
import { undoComment } from "../annotate/edit";
import { undoDepth } from "../annotate/state";
import type { EditTool } from "./geometry";
import { editTool, setEditTool } from "./state";
import { et } from "./text";

/** Barra del modo Editar: editar un párrafo o añadir texto, y deshacer (la pila es la de Comentar). */
export function EditToolbar() {
  return (
    <>
      <ToggleGroup label={et("edit.tool.label")} value={editTool()} disabled={!activeDoc()}
        options={(["text", "add"] as EditTool[]).map((t) => ({ value: t, label: et("edit.tool." + t) }))} onChange={(v) => setEditTool(v as EditTool)} />
      <ToolbarSeparator />
      <IconButton icon="undo" label={et("edit.undo")} disabled={undoDepth() === 0 || !activeDoc()} onClick={() => activeDoc() && void undoComment(activeDoc()!)} />
      <span class="pl-edit-hint">{et("edit.hint." + editTool())}</span>
    </>
  );
}
