import { activeDoc } from "../../state/documents";
import { Checkbox, IconButton, ToggleGroup, ToolbarSeparator } from "../../ui";
import { undoComment } from "../annotate/edit";
import { undoDepth } from "../annotate/state";
import type { EditTool } from "./geometry";
import { editTool, reflow, setEditTool, setReflow } from "./state";
import { et } from "./text";

const TOOLS: EditTool[] = ["text", "add", "image", "object", "link"];

/** Barra del modo Editar: texto, imágenes, objetos vectoriales y enlaces; recolocar lo de debajo; y deshacer (la pila es la de Comentar). */
export function EditToolbar() {
  return (
    <>
      <ToggleGroup label={et("edit.tool.label")} value={editTool()} disabled={!activeDoc()}
        options={TOOLS.map((t) => ({ value: t, label: et("edit.tool." + t) }))} onChange={(v) => setEditTool(v as EditTool)} />
      <ToolbarSeparator />
      <Checkbox label={et("edit.reflow")} hint={undefined} checked={reflow()} disabled={!activeDoc()} onChange={(e) => setReflow(e.currentTarget.checked)} />
      <ToolbarSeparator />
      <IconButton icon="undo" label={et("edit.undo")} disabled={undoDepth() === 0 || !activeDoc()} onClick={() => activeDoc() && void undoComment(activeDoc()!)} />
      <span class="pl-edit-hint">{et("edit.hint." + editTool())}</span>
    </>
  );
}
