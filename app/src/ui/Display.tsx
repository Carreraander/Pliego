import { For, Show, type JSX } from "solid-js";
import { Icon } from "./Icon";
import { ut } from "./strings";
import "./Display.css";

/** Atajo de teclado: <Kbd keys="Ctrl+K" /> → Ctrl K. */
export function Kbd(props: { keys: string }) {
  return (
    <span class="pl-kbd-group">
      <For each={props.keys.split("+")}>{(k) => <kbd class="pl-kbd">{k}</kbd>}</For>
    </span>
  );
}

export type Tone = "neutral" | "accent" | "ok" | "warn" | "danger" | "info";

export function Chip(props: { tone?: Tone; icon?: string; children: JSX.Element; onClick?: () => void; title?: string }) {
  const cls = () => `pl-chip pl-chip--${props.tone ?? "neutral"}`;
  const body = () => <>{props.icon && <Icon name={props.icon} size={14} />}{props.children}</>;
  return (
    <Show when={props.onClick} fallback={<span class={cls()} title={props.title}>{body()}</span>}>
      <button type="button" class={`${cls()} pl-chip--action`} onClick={() => props.onClick?.()} title={props.title}>{body()}</button>
    </Show>
  );
}

export type NoticeTone = "info" | "ok" | "warn" | "danger";
const noticeIcon: Record<NoticeTone, string> = { info: "info", ok: "check-circle", warn: "alert-triangle", danger: "x-circle" };

export interface NoticeAction { label: string; onAction: () => void }

/** Barra de aviso (36 px): texto llano + una acción (§5.2 del diseño). `inline` la usa dentro de paneles. */
export function Notice(props: { tone?: NoticeTone; actions?: NoticeAction[]; onDismiss?: () => void; inline?: boolean; children: JSX.Element }) {
  const tone = () => props.tone ?? "info";
  return (
    <div class={`pl-notice pl-notice--${tone()}`} classList={{ "pl-notice--inline": props.inline }} role={tone() === "danger" ? "alert" : "status"}>
      <Icon name={noticeIcon[tone()]} size={16} />
      <span class="pl-notice__text">{props.children}</span>
      <div class="pl-notice__actions">
        <For each={props.actions}>{(a) => <button type="button" class="pl-notice__action" onClick={() => a.onAction()}>{a.label}</button>}</For>
        <Show when={props.onDismiss}>
          <button type="button" class="pl-notice__close" aria-label={ut("ui.closeNotice")} onClick={() => props.onDismiss?.()}><Icon name="x" size={14} /></button>
        </Show>
      </div>
    </div>
  );
}

export function EmptyState(props: { icon?: string; title: string; hint?: string; children?: JSX.Element }) {
  return (
    <div class="pl-empty">
      <Show when={props.icon}><Icon name={props.icon!} size={32} /></Show>
      <h2>{props.title}</h2>
      <Show when={props.hint}><p class="pl-muted">{props.hint}</p></Show>
      {props.children}
    </div>
  );
}
