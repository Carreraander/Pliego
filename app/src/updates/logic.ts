import type { UpdateSnapshot } from "../settings/updates";

/** ¿Qué muestra la pastilla de la barra de título? `null` = nada. */
export function pillKind(s: UpdateSnapshot | null): "ready" | "available" | null {
  if (!s) return null;
  if (s.phase === "ready") return "ready";
  if (s.phase === "available" && !s.mandatory && s.can_download) return "available";
  return null;
}

/** ¿Hay que mostrar la barra de aviso de actualización obligatoria? */
export function mandatoryNotice(s: UpdateSnapshot | null): { version: string; ready: boolean } | null {
  if (!s) return null;
  if ((s.phase === "ready" || s.phase === "available") && s.mandatory) return { version: s.version, ready: s.phase === "ready" };
  return null;
}

/** Porcentaje de la descarga, o `null` si no se conoce. */
export function downloadPercent(s: UpdateSnapshot | null): number | null {
  return s && s.phase === "downloading" && s.total > 0 ? Math.min(100, Math.round((s.done / s.total) * 100)) : null;
}
