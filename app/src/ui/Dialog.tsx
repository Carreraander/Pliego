import { createEffect, createUniqueId, on, type JSX } from "solid-js";
import "./Dialog.css";

/**
 * Diálogo modal sobre `<dialog>` nativo (trampa de foco y `Esc` gratis). Solo para preguntas del sistema:
 * contraseña de apertura, confirmar cierre con cambios (§4.1: las tareas nunca abren un modal).
 */
export function Dialog(props: { open: boolean; title: string; onClose: () => void; children: JSX.Element; actions?: JSX.Element; onSubmit?: () => void }) {
  let el!: HTMLDialogElement;
  const id = createUniqueId();
  createEffect(on(() => props.open, (open) => {
    if (open && !el.open) el.showModal();
    else if (!open && el.open) el.close();
  }));
  return (
    <dialog ref={el} class="pl-dialog" aria-labelledby={id} onCancel={(e) => { e.preventDefault(); props.onClose(); }}>
      <form method="dialog" class="pl-dialog__form" onSubmit={(e) => { e.preventDefault(); props.onSubmit?.(); }}>
        <h2 id={id}>{props.title}</h2>
        <div class="pl-dialog__body">{props.children}</div>
        <div class="pl-dialog__actions">{props.actions}</div>
      </form>
    </dialog>
  );
}
