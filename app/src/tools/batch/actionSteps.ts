/** Acción guardada tal como la devuelve `actions_load` (`pliego_actions::Action`). */
export interface SavedAction { name: string; description: string; on_error: string; steps: ({ kind: string } & Record<string, unknown>)[] }

/** Resumen de un paso para la lista: «pages.rotate (degrees = 90)». */
export function stepLabel(step: SavedAction["steps"][number]): string {
  const { kind, ...opts } = step;
  const parts = Object.entries(opts)
    .filter(([, v]) => v !== null && v !== undefined && typeof v !== "object")
    .map(([k, v]) => `${k} = ${String(v)}`);
  return parts.length ? `${kind} (${parts.join(", ")})` : kind;
}
