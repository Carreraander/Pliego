import { createSignal, createUniqueId, For, Show, type JSX } from "solid-js";
import { Button, IconButton, type ButtonVariant } from "./Button";
import { Icon } from "./Icon";
import { ut } from "./strings";
import "./Panels.css";

/**
 * Contenedor del inspector (columna derecha, 280–480 px; el armazón decide el ancho). Propiedades de lo seleccionado
 * o panel de la tarea en curso. `title` es el nombre accesible de la región.
 */
export function Inspector(props: { title: string; onClose?: () => void; actions?: JSX.Element; children: JSX.Element; footer?: JSX.Element; class?: string }) {
  const id = createUniqueId();
  return (
    <section class={`pl-inspector ${props.class ?? ""}`} aria-labelledby={id}>
      <header class="pl-inspector__head">
        <h2 id={id}>{props.title}</h2>
        {props.actions}
        <Show when={props.onClose}><IconButton icon="x" label={ut("ui.close")} size="sm" onClick={() => props.onClose?.()} /></Show>
      </header>
      <div class="pl-inspector__body">{props.children}</div>
      <Show when={props.footer}><footer class="pl-inspector__foot">{props.footer}</footer></Show>
    </section>
  );
}

export interface TaskAction { label: string; onClick: () => void; variant?: ButtonVariant; disabled?: boolean; icon?: string }
export interface TaskProgress {
  /** 0–1; `null` = indeterminado. */
  value: number | null;
  label: string;
  onCancel?: () => void;
}

/**
 * Panel de tarea (§5.1 del diseño): título y cierre, opciones simples, «Opciones avanzadas» plegadas, estimación y pie con
 * acciones. El botón primario es la opción segura (guardar copia). Con `progress` las opciones se bloquean y el pie
 * muestra el avance; el panel se puede cerrar y la tarea sigue en el centro de tareas.
 */
export function TaskPanel(props: {
  title: string;
  onClose?: () => void;
  children: JSX.Element;
  advanced?: JSX.Element;
  /** Texto de estimación o vista previa del efecto («Ahora 12,4 MB → estimado ~3,1 MB»). */
  estimate?: JSX.Element;
  primary: TaskAction;
  secondary?: TaskAction[];
  progress?: TaskProgress;
  /** Aviso o error en línea, en el panel donde ocurrió. */
  notice?: JSX.Element;
}) {
  const [showAdvanced, setShowAdvanced] = createSignal(false);
  const advId = createUniqueId();
  const busy = () => props.progress !== undefined;
  return (
    <Inspector
      title={props.title} onClose={props.onClose}
      footer={
        <Show when={!busy()} fallback={<ProgressBar {...props.progress!} />}>
          <Show when={props.estimate}><p class="pl-task__estimate pl-num">{props.estimate}</p></Show>
          <div class="pl-task__actions">
            <Button variant={props.primary.variant ?? "primary"} icon={props.primary.icon} disabled={props.primary.disabled} onClick={() => props.primary.onClick()}>{props.primary.label}</Button>
            <For each={props.secondary}>{(a) => <Button variant={a.variant ?? "normal"} icon={a.icon} disabled={a.disabled} onClick={() => a.onClick()}>{a.label}</Button>}</For>
          </div>
        </Show>
      }
    >
      <Show when={props.notice}><div class="pl-task__notice">{props.notice}</div></Show>
      <fieldset class="pl-task__options" disabled={busy()}>
        {props.children}
        <Show when={props.advanced}>
          <button type="button" class="pl-task__adv" aria-expanded={showAdvanced()} aria-controls={advId} onClick={() => setShowAdvanced(!showAdvanced())}>
            <Icon name={showAdvanced() ? "chevron-down" : "chevron-right"} size={16} />{ut("ui.advanced")}
          </button>
          <div id={advId} class="pl-task__advbody" hidden={!showAdvanced()}>{props.advanced}</div>
        </Show>
      </fieldset>
    </Inspector>
  );
}

export function ProgressBar(props: TaskProgress) {
  const pct = () => (props.value === null ? undefined : Math.round(props.value * 100));
  return (
    <div class="pl-progress">
      <div class="pl-progress__row">
        <span class="pl-progress__label" id="pl-progress-label">{props.label}</span>
        <Show when={pct() !== undefined}><span class="pl-num pl-muted">{pct()} %</span></Show>
        <Show when={props.onCancel}><Button variant="ghost" onClick={() => props.onCancel?.()}>{ut("ui.cancel")}</Button></Show>
      </div>
      <div class="pl-progress__track" role="progressbar" aria-label={props.label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct()}>
        <div class="pl-progress__bar" classList={{ "pl-progress__bar--indeterminate": props.value === null }} style={{ width: pct() === undefined ? undefined : `${pct()}%` }} />
      </div>
    </div>
  );
}

/** Pasos de un asistente (§4.1): lista ordenada con el paso actual marcado. Los botones Atrás/Siguiente van en el pie del panel. */
export function WizardSteps(props: { steps: string[]; current: number; class?: string }) {
  return (
    <ol class={`pl-steps ${props.class ?? ""}`} aria-label={ut("ui.step", { n: props.current + 1, total: props.steps.length })}>
      <For each={props.steps}>{(label, i) => (
        <li class="pl-steps__item" classList={{ "pl-steps__item--done": i() < props.current, "pl-steps__item--current": i() === props.current }} aria-current={i() === props.current ? "step" : undefined}>
          <span class="pl-steps__dot" aria-hidden="true">{i() < props.current ? <Icon name="check" size={12} /> : i() + 1}</span>
          <span class="pl-steps__label">{label}</span>
        </li>
      )}</For>
    </ol>
  );
}
