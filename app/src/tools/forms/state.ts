import { createSignal } from "solid-js";
import type { DesignTool, FieldInfo } from "./geometry";

/** Herramienta activa del diseñador (`select` = elegir y editar campos existentes). */
export const [designTool, setDesignTool] = createSignal<DesignTool>("select");
/** Campo elegido (por nombre) para editar sus propiedades. */
export const [selectedField, setSelectedField] = createSignal<string | null>(null);
/** Campos del documento activo para la revisión mostrada. */
export const [fieldList, setFieldList] = createSignal<FieldInfo[]>([]);
