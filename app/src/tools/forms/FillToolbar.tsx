import { Show } from "solid-js";
import { activeDoc } from "../../state/documents";
import { Button, IconButton, ToolbarSeparator } from "../../ui";
import { undoComment } from "../annotate/edit";
import { undoDepth } from "../annotate/state";
import { at } from "../annotate/text";
import { SignaturePad } from "./SignaturePad";
import { placingSignature, setPlacingSignature, setSignaturePadOpen, signature, signaturePadOpen } from "./state";
import { ft } from "./text";

/** Barra del modo Rellenar y firmar: «Mi firma» y deshacer. */
export function FillToolbar() {
  return (
    <>
      <Button variant="ghost" icon="signature" disabled={!activeDoc()} aria-pressed={placingSignature()}
        onClick={() => (signature() ? setPlacingSignature(!placingSignature()) : setSignaturePadOpen(true))}>
        {signature() ? ft("forms.signature.place") : ft("forms.mySignature")}
      </Button>
      <Show when={signature()}>
        <Button variant="ghost" icon="pencil" disabled={!activeDoc()} onClick={() => setSignaturePadOpen(true)}>{ft("forms.signature.edit")}</Button>
      </Show>
      <Show when={placingSignature()}><span class="pl-muted">{ft("forms.signature.placing")}</span></Show>
      <ToolbarSeparator />
      <IconButton icon="undo" label={at("annotate.undo")} disabled={undoDepth() === 0 || !activeDoc()} onClick={() => activeDoc() && void undoComment(activeDoc()!)} />
      <Show when={signaturePadOpen()}>
        <div class="pl-signature-pop"><SignaturePad /></div>
      </Show>
    </>
  );
}
