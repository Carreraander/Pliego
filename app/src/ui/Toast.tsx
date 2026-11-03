import { createSignal, For, onCleanup, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { Icon } from "./Icon";
import { IconButton } from "./Button";
import { ut } from "./strings";
import type { NoticeTone } from "./Display";
import "./Overlay.css";

export interface ToastOptions { tone?: NoticeTone; text: string; action?: { label: string; onAction: () => void }; /** ms; 0 = hasta que se cierre. Por defecto 5000 (errores: no se cierran solos). */ timeout?: number }
interface ToastItem extends ToastOptions { id: number }

const [toasts, setToasts] = createSignal<ToastItem[]>([]);
let next = 1;

/** Muestra un aviso breve no bloqueante. Devuelve una función para cerrarlo. */
export function toast(options: ToastOptions): () => void {
  const id = next++;
  setToasts((l) => [...l, { ...options, id }].slice(-4));
  return () => dismiss(id);
}
export function dismiss(id: number): void { setToasts((l) => l.filter((t) => t.id !== id)); }
export function clearToasts(): void { setToasts([]); }

const icons: Record<NoticeTone, string> = { info: "info", ok: "check-circle", warn: "alert-triangle", danger: "x-circle" };

function ToastView(props: { item: ToastItem }) {
  const tone = () => props.item.tone ?? "info";
  const timeout = () => props.item.timeout ?? (tone() === "danger" ? 0 : 5000);
  let timer: number | undefined;
  const arm = () => { clearTimeout(timer); if (timeout() > 0) timer = window.setTimeout(() => dismiss(props.item.id), timeout()); };
  arm();
  onCleanup(() => clearTimeout(timer));
  return (
    <div class={`pl-toast pl-toast--${tone()}`} role={tone() === "danger" ? "alert" : "status"} onPointerEnter={() => clearTimeout(timer)} onPointerLeave={arm} onFocusIn={() => clearTimeout(timer)} onFocusOut={arm}>
      <Icon name={icons[tone()]} size={16} />
      <span class="pl-toast__text">{props.item.text}</span>
      <Show when={props.item.action}>{(a) => <button type="button" class="pl-notice__action" onClick={() => { a().onAction(); dismiss(props.item.id); }}>{a().label}</button>}</Show>
      <IconButton icon="x" size="sm" label={ut("ui.close")} onClick={() => dismiss(props.item.id)} />
    </div>
  );
}

/** Región de avisos: móntala una vez en el armazón. */
export function ToastRegion() {
  return (
    <Portal>
      <div class="pl-toasts"><For each={toasts()}>{(item) => <ToastView item={item} />}</For></div>
    </Portal>
  );
}
