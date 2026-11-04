import { createEffect, createRoot, on } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { activeDoc } from "../../state/documents";
import { dismissNotice, registerPanel, setActivePanel, setSideOpen, showNotice } from "../../shell";
import { setDocFacts } from "../registry";
import { SignaturesPanel } from "./SignaturesPanel";
import { overall, verdictTone, type SignatureReport } from "./status";
import { st } from "./text";

import { PANEL_ID } from "./panel-id";
export { PANEL_ID };
const NOTICE_ID = "sign.summary";

/** Firmas del archivo (vacío si no tiene o no se puede leer: no es un error para quien solo abre el documento). */
export async function validateDoc(path: string): Promise<SignatureReport[]> {
  return invoke<SignatureReport[]>("sign_validate", { path }).catch(() => []);
}

/** Texto del aviso del documento a partir de sus firmas. */
export function noticeText(reports: SignatureReport[]): string {
  const v = overall(reports)!;
  const first = reports[0]!;
  if (reports.length === 1) return st(`notice.${v}`, { name: first.signer });
  return st("notice.many", { n: reports.length, state: st(`verdict.${v}`) });
}

/**
 * Registra el panel «Firmas» del raíl y mantiene al día los hechos del documento activo (`signatures`; `document` lo fija la navegación) y el
 * aviso «Firmado por…». Devuelve cómo desmontarlo.
 */
export function installSignatures(): () => void {
  // El panel solo existe mientras el documento activo tenga firmas (así el raíl no ofrece un panel vacío).
  let offPanel: (() => void) | null = null;
  const setPanel = (on: boolean) => {
    if (on && !offPanel) offPanel = registerPanel({ id: PANEL_ID, title: st("validate.title"), icon: "badge-check", order: 60, component: SignaturesPanel });
    if (!on && offPanel) { offPanel(); offPanel = null; }
  };
  let shown: { docId: string } | null = null;
  const clearNotice = () => { if (shown) { dismissNotice(shown.docId, NOTICE_ID); shown = null; } };
  const dispose = createRoot((d) => {
    createEffect(on(
      () => { const doc = activeDoc(); return doc ? { id: String(doc.id), path: doc.path, revision: doc.revision } : null; },
      async (key) => {
        clearNotice();
        setPanel(false);
        if (!key) { setDocFacts({ signatures: false }); return; }
        setDocFacts({ signatures: false });
        const reports = await validateDoc(key.path);
        if (String(activeDoc()?.id) !== key.id) return; // se cambió de documento mientras tanto
        setDocFacts({ signatures: reports.length > 0 });
        setPanel(reports.length > 0);
        if (!reports.length) return;
        const v = overall(reports)!;
        showNotice(key.id, {
          id: NOTICE_ID, kind: "signed", tone: verdictTone(v) === "ok" ? "ok" : verdictTone(v) === "warn" ? "warn" : "danger",
          text: noticeText(reports), dismissible: true,
          actions: [{ label: st("notice.view"), onAction: () => { setActivePanel(PANEL_ID); setSideOpen(true); } }],
        });
        shown = { docId: key.id };
      },
    ));
    return d;
  });
  return () => { setPanel(false); clearNotice(); dispose(); };
}

