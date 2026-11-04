import { activeDoc } from "../../state/documents";
import { IconButton, ToggleGroup, ToolbarSeparator } from "../../ui";
import { undoComment } from "../annotate/edit";
import { undoDepth } from "../annotate/state";
import type { DesignTool } from "./geometry";
import { designTool, setDesignTool } from "./state";
import { ft } from "./text";

const TOOLS: DesignTool[] = ["select", "text", "check_box", "radio_button", "combo_box", "list_box", "push_button", "signature"];

/** Barra del modo Diseñar formulario: tipo de campo a dibujar y deshacer (la pila es la de Comentar). */
export function FormDesignToolbar() {
  return (
    <>
      <ToggleGroup label={ft("forms.tool.label")} value={designTool()} disabled={!activeDoc()}
        options={TOOLS.map((t) => ({ value: t, label: ft("forms.tool." + t) }))} onChange={(v) => setDesignTool(v as DesignTool)} />
      <ToolbarSeparator />
      <IconButton icon="undo" label={ft("forms.undo")} disabled={undoDepth() === 0 || !activeDoc()} onClick={() => activeDoc() && void undoComment(activeDoc()!)} />
      <span class="pl-forms-hint">{ft("forms.hint")}</span>
    </>
  );
}
