import { createResource } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import { activeDoc, type OpenDoc } from "../../state/documents";
import { loadSignature, type Signature } from "./signature";
import { withPassword } from "../../state/passwords";

/** Reflejo de `pliego_forms::FieldInfo` (lo que se usa). */
export interface FormField {
  name: string; kind: "text" | "check_box" | "radio_button" | "combo_box" | "list_box" | "push_button" | "signature" | "unknown";
  value: string | null; options: string[]; page: number; rect: [number, number, number, number] | null;
  read_only: boolean; multiline: boolean; password: boolean; signed: boolean;
}

/** Campos del documento activo; se vuelven a leer cuando cambia su revisión (cada relleno recarga el visor). */
export const [fields, { refetch: refetchFields }] = createResource(
  () => { const d = activeDoc(); return d ? ({ path: d.path, rev: d.revision } satisfies { path: string; rev: number }) : null; },
  (k) => invoke<FormField[]>("forms_fields", withPassword({ path: k.path })).catch(() => [] as FormField[]),
);

export const fieldsOnPage = (doc: OpenDoc, page: number): FormField[] =>
  doc.id === activeDoc()?.id ? (fields() ?? []).filter((f) => f.page === page && f.rect) : [];

/** Firma guardada (`null` si no hay) y si se está colocando. */
export const [signature, setSignature] = createSignal<Signature | null>(loadSignature());
export const [placingSignature, setPlacingSignature] = createSignal(false);
export const [signaturePadOpen, setSignaturePadOpen] = createSignal(false);
