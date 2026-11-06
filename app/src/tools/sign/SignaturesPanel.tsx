import "./sign.css";
import { createResource, For, Show } from "solid-js";
import { Button, Chip, EmptyState } from "../../ui";
import { docs } from "../../state/documents";
import { validateDoc } from "./facts";
import { levelLabel, problemLines, verdict, verdictTone, type SignatureReport } from "./status";
import { st } from "./text";

function Card(p: { r: SignatureReport }) {
  const v = () => verdict(p.r);
  return (
    <li class="pl-sign-card">
      <h3>{p.r.signer}</h3>
      <div><Chip tone={verdictTone(v())} icon={v() === "valid" ? "check-circle" : v() === "unknown-issuer" ? "alert-triangle" : "x-circle"}>{st(`verdict.${v()}`)}</Chip></div>
      <ul>
        <li>{p.r.integrity ? st("validate.integrity") : st("validate.noIntegrity")}</li>
        <li>{p.r.covers_whole_document ? st("validate.whole") : st("validate.partial")}</li>
        <li>{st(`validate.chain.${p.r.chain}`)}</li>
        <Show when={p.r.has_timestamp}><li>{st("validate.timestamp")}</li></Show>
        <Show when={p.r.has_dss}><li>{st("validate.dss")}</li></Show>
        <Show when={p.r.certification}><li>{st("validate.certification", { n: p.r.certification! })}</li></Show>
        <li>{st("validate.level", { level: levelLabel(p.r.level) })}</li>
        <For each={problemLines(p.r, st)}>{(m) => <li>{m}</li>}</For>
      </ul>
      <Show when={p.r.signing_time}><span class="pl-muted">{st("validate.signedAt", { date: p.r.signing_time! })}</span></Show>
      <Show when={p.r.reason}><span class="pl-muted">{st("validate.reason", { text: p.r.reason! })}</span></Show>
      <Show when={p.r.location}><span class="pl-muted">{st("validate.location", { text: p.r.location! })}</span></Show>
    </li>
  );
}

/** Panel del raíl «Firmas»: estado de cada firma del documento activo. */
export function SignaturesPanel(props: { docId: string | null }) {
  const doc = () => docs().find((d) => String(d.id) === props.docId) ?? null;
  const [reports, { refetch }] = createResource(
    () => { const d = doc(); return d ? { path: d.path, revision: d.revision } : null; },
    (k) => validateDoc(k.path),
  );
  return (
    <div class="pl-sign-panel">
      <Show when={!reports.loading} fallback={<p class="pl-muted">{st("validate.loading")}</p>}>
        <Show when={reports()?.length} fallback={<EmptyState icon="badge-check" title={st("validate.none")} />}>
          <ul class="pl-sign-list"><For each={reports()}>{(r) => <Card r={r} />}</For></ul>
        </Show>
      </Show>
      <Button icon="refresh" onClick={() => void refetch()}>{st("validate.refresh")}</Button>
    </div>
  );
}
