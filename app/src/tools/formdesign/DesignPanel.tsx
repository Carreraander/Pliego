import "./formdesign.css";
import { onCleanup, onMount, Show } from "solid-js";
import { activeDoc } from "../../state/documents";
import { TaskPanel, ToggleGroup } from "../../ui";
import { undoComment } from "../annotate/edit";
import { undoDepth } from "../annotate/state";
import FieldPanel from "./FieldPanel";
import type { DesignTool } from "./geometry";
import { designTool, editing, fieldList, setDesignOpen, setDesignTool, setEditing, setSelectedField } from "./state";
import { ft } from "./text";

const TOOLS: DesignTool[] = ["select", "text", "check_box", "radio_button", "combo_box", "list_box", "push_button", "signature"];

/**
 * Diseñar formulario: panel del inspector que activa la capa del visor. Elegir tipo de campo, dibujarlo en la página y
 * editar sus propiedades aquí mismo. Es una tarea (no un modo) para no añadir un séptimo modo a la barra.
 */
export default function DesignPanel(p: { onClose: () => void }) {
  onMount(() => setDesignOpen(true));
  onCleanup(() => { setDesignOpen(false); setEditing(null); setSelectedField(null); setDesignTool("select"); });
  const doc = activeDoc;
  return (
    <Show when={editing()} keyed fallback={
      <TaskPanel title={ft("tools.forms.design.title")} onClose={p.onClose} primary={{ label: ft("fdesign.panel.close"), variant: "normal", onClick: p.onClose }}
        secondary={[{ label: ft("fdesign.undo"), icon: "undo", disabled: undoDepth() === 0 || !doc(), onClick: () => doc() && void undoComment(doc()!) }]}
        estimate={ft("fdesign.count", { n: fieldList().length })}>
        <ToggleGroup label={ft("fdesign.tool.label")} value={designTool()} disabled={!doc()} class="pl-fdesign-tools"
          options={TOOLS.map((t) => ({ value: t, label: ft("fdesign.tool." + t) }))} onChange={(v) => setDesignTool(v as DesignTool)} />
        <p class="pl-fdesign-hint">{ft("fdesign.hint")}</p>
      </TaskPanel>
    }>
      {(e) => <FieldPanel editing={e} onClose={() => setEditing(null)} />}
    </Show>
  );
}
