import { withPassword } from "../../state/passwords";
import "./measure.css";
import { createEffect, createResource, createSignal, onCleanup, onMount, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { activeDoc } from "../../state/documents";
import { Button, Field, Notice, Select, TaskPanel, TextInput, ToggleGroup, toast } from "../../ui";
import { undoComment } from "../annotate/edit";
import { setUndoDepth, undoDepth } from "../annotate/state";
import { errorText } from "../run";
import { minPoints, pageDistance, parsePositive, UNITS, type Measurement, type MeasureMode, type Scale, type Unit } from "./geometry";
import { activeScale, marks, measureMode, setActiveScale, setMarks, setMeasureMode, setMeasureOpen } from "./state";
import { mt } from "./text";

const MODES: MeasureMode[] = ["calibrate", "distance", "area"];
const trim = (v: number) => v.toFixed(6).replace(/0+$/, "").replace(/\.$/, "");

/** Medir sobre planos: calibrar la escala, medir distancias o áreas con clics y guardar la medida como anotación. */
export default function MeasurePanel(p: { onClose: () => void }) {
  onMount(() => setMeasureOpen(true));
  onCleanup(() => { setMeasureOpen(false); setMarks({ page: 0, points: [] }); setMeasureMode("distance"); });
  const [real, setReal] = createSignal("");
  const [ratio, setRatio] = createSignal("");
  const [unit, setUnit] = createSignal<Unit>("m");
  const [error, setError] = createSignal<string | null>(null);
  const fail = (e: unknown) => setError(mt("measure.failed", { error: errorText(e) }));

  // La escala guardada en la página (si la hay) se usa al abrir o al cambiar de página con marcas.
  createEffect(() => {
    const doc = activeDoc();
    if (!doc) return;
    invoke<Scale | null>("measure_page_scale", withPassword({ path: doc.path, page: marks().page }))
      .then((s) => { if (s) setActiveScale(s); })
      .catch(fail);
  });

  const closed = () => measureMode() === "area";
  const ready = () => measureMode() !== "calibrate" && !!activeScale() && marks().points.length >= minPoints(measureMode());
  const [live] = createResource(
    () => (ready() ? { scale: activeScale()!, points: marks().points, closed: closed() } : null),
    (a) => invoke<Measurement>("measure_shape", a).catch((e) => { fail(e); return null; }),
  );

  const apply = async () => {
    setError(null);
    const v = parsePositive(real());
    const pts = marks().points;
    const [a, b] = pts;
    if (v === null || !a || !b || pts.length !== 2) { setError(mt("measure.bad")); return; }
    try {
      setActiveScale(await invoke<Scale>("measure_calibrate", { pagePoints: pageDistance(a, b), real: v, unit: unit() }));
      setMarks({ page: marks().page, points: [] }); setReal(""); setMeasureMode("distance");
    } catch (e) { fail(e); }
  };
  const useRatio = async () => {
    setError(null);
    const n = parsePositive(ratio());
    if (n === null) { setError(mt("measure.bad")); return; }
    try { setActiveScale(await invoke<Scale>("measure_ratio", { n, unit: unit() })); } catch (e) { fail(e); }
  };
  const save = async () => {
    const doc = activeDoc(); const scale = activeScale();
    if (!doc || !scale || !ready()) return;
    setError(null);
    try {
      const r = await invoke<{ measurement: Measurement; undo_depth: number }>("measure_save", {
        docId: doc.id, path: doc.path, page: marks().page, scale, points: marks().points, closed: closed(), suffix: mt("measure.backupSuffix"),
      });
      setUndoDepth(r.undo_depth);
      toast({ tone: "ok", text: mt("measure.saved", { text: r.measurement.text }) });
      setMarks({ page: marks().page, points: [] });
    } catch (e) { fail(e); }
  };
  const clear = () => setMarks({ page: marks().page, points: [] });
  const calibrating = () => measureMode() === "calibrate";

  return (
    <TaskPanel
      title={mt("measure.title")} onClose={p.onClose}
      primary={calibrating()
        ? { label: mt("measure.apply"), icon: "check-circle", disabled: !activeDoc() || marks().points.length !== 2, onClick: () => void apply() }
        : { label: mt("measure.save"), icon: "check-circle", disabled: !ready(), onClick: () => void save() }}
      secondary={[
        { label: mt("measure.clear"), icon: "x", disabled: marks().points.length === 0, onClick: clear },
        { label: mt("measure.undo"), icon: "undo", disabled: undoDepth() === 0 || !activeDoc(), onClick: () => activeDoc() && void undoComment(activeDoc()!) },
      ]}
      notice={<Show when={!activeDoc() || error()}><Notice tone={error() ? "danger" : "warn"} inline>{error() ?? mt("measure.noDoc")}</Notice></Show>}
    >
      <ToggleGroup label={mt("measure.mode")} value={measureMode()} disabled={!activeDoc()}
        options={MODES.map((m) => ({ value: m, label: mt(`measure.mode.${m}`) }))} onChange={(v) => { setMeasureMode(v as MeasureMode); clear(); }} />
      <p class="pl-measure-hint">{mt(`measure.hint.${measureMode()}`)}</p>
      <Show when={calibrating()}>
        <Field label={mt("measure.real")}><TextInput inputmode="decimal" value={real()} onInput={(e) => setReal(e.currentTarget.value)} /></Field>
      </Show>
      <Field label={mt("measure.unit")}>
        <Select value={unit()} onChange={(e) => setUnit(e.currentTarget.value as Unit)} options={UNITS.map((u) => ({ value: u, label: u }))} />
      </Field>
      <Show when={calibrating()}>
        <Field label={mt("measure.ratio")}>
          <TextInput inputmode="numeric" value={ratio()} onInput={(e) => setRatio(e.currentTarget.value)} placeholder="100" />
        </Field>
        <Button onClick={() => void useRatio()}>{mt("measure.ratio.apply")}</Button>
      </Show>
      <p class="pl-measure-hint">
        {mt("measure.scale")}: {activeScale() ? mt("measure.scale.current", { ratio: `${trim(activeScale()!.units_per_point)} ${activeScale()!.unit}` }) : mt("measure.scale.none")}
      </p>
      <Show when={live()}>{(m) => <p class="pl-measure-result" role="status">{mt("measure.result")}: {m().text}</p>}</Show>
    </TaskPanel>
  );
}
