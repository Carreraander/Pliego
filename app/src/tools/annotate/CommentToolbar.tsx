import { For } from "solid-js";
import { activeDoc } from "../../state/documents";
import { IconButton, ToggleGroup, ToolbarSeparator } from "../../ui";
import { undoComment } from "./edit";
import { COLORS, type CommentTool } from "./geometry";
import { commentColor, commentTool, setCommentColor, setCommentTool, undoDepth } from "./state";
import { at } from "./text";

const TOOLS: { value: CommentTool; icon: string; key: string }[] = [
  { value: "highlight", icon: "highlighter", key: "annotate.tool.highlight" },
  { value: "underline", icon: "underline", key: "annotate.tool.underline" },
  { value: "strike", icon: "minus", key: "annotate.tool.strike" },
  { value: "note", icon: "message-square", key: "annotate.tool.note" },
  { value: "ink", icon: "pen-line", key: "annotate.tool.ink" },
];

/** Barra del modo Comentar: herramienta, color y deshacer. */
export function CommentToolbar() {
  return (
    <>
      <ToggleGroup label={at("annotate.tool.label")} value={commentTool() ?? ""} disabled={!activeDoc()}
        options={TOOLS.map((o) => ({ value: o.value, icon: o.icon, title: at(o.key) }))} onChange={(v) => setCommentTool(v as CommentTool)} />
      <ToolbarSeparator />
      <div role="radiogroup" aria-label={at("annotate.color.label")} style={{ display: "flex", gap: "6px", "align-items": "center" }}>
        <For each={COLORS}>{(c) => (
          <button type="button" role="radio" class="pl-comment-swatch" title={at(`annotate.color.${c.id}`)} aria-label={at(`annotate.color.${c.id}`)}
            aria-checked={commentColor() === c.rgb} style={{ background: `rgb(${c.rgb.map((v) => Math.round(v * 255)).join(" ")})` }} onClick={() => setCommentColor(c.rgb)} />
        )}</For>
      </div>
      <ToolbarSeparator />
      <IconButton icon="undo" label={at("annotate.undo")} disabled={undoDepth() === 0 || !activeDoc()} onClick={() => activeDoc() && void undoComment(activeDoc()!)} />
    </>
  );
}
