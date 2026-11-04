import { createSignal } from "solid-js";
import type { OpenDoc } from "../../state/documents";
import type { Box, DesignTool, FieldInfo, FieldKind } from "./geometry";

/** Herramienta activa del diseñador (`select` = elegir y editar campos existentes). */
export const [designTool, setDesignTool] = createSignal<DesignTool>("select");
/** El panel «Diseñar formulario» está abierto: solo entonces la capa del visor recoge gestos. */
export const [designOpen, setDesignOpen] = createSignal(false);
/** Campo elegido (por nombre) para editar sus propiedades. */
export const [selectedField, setSelectedField] = createSignal<string | null>(null);
/** Campos del documento activo para la revisión mostrada. */
export const [fieldList, setFieldList] = createSignal<FieldInfo[]>([]);

/** Lo que se está creando o editando en el panel (`null` = elegir herramienta). */
export type Editing =
  | { mode: "new"; doc: OpenDoc; kind: FieldKind; page: number; rect: Box }
  | { mode: "edit"; doc: OpenDoc; field: FieldInfo };
export const [editing, setEditing] = createSignal<Editing | null>(null);
