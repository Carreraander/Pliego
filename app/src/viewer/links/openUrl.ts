import { createSignal } from "solid-js";
import { api } from "../../api";
import { t } from "../../i18n";
import { engineText } from "../../i18n/engine";
import { toast } from "../../ui";

/** Dirección a la espera de que la persona confirme que se abra (`null` = ninguna). */
const [pending, setPending] = createSignal<string | null>(null);
export const pendingUrl = pending;

/**
 * Pide abrir una dirección que viene de un documento (enlace de una página, marcador…). **Nunca se abre sin confirmar**:
 * un PDF no fiable puede llevar a cualquier sitio. Solo `http`, `https` y `mailto`; lo demás se rechaza ya aquí con un
 * aviso (y otra vez en el lado nativo).
 */
export function requestOpenUrl(url: string): void {
  if (!/^(https?:\/\/|mailto:)/i.test(url.trim())) {
    toast({ tone: "warn", text: t("link.failed", { error: url.trim().split(":")[0] || "?" }) });
    return;
  }
  setPending(url.trim());
}

export function cancelOpenUrl(): void { setPending(null); }

/** La persona ha confirmado: se abre en el sistema. */
export async function confirmOpenUrl(): Promise<void> {
  const url = pending();
  setPending(null);
  if (!url) return;
  try {
    await api.openWebLink(url);
  } catch (e) {
    const error = typeof e === "string" ? e : e instanceof Error ? e.message : (e as { message?: string })?.message ?? "?";
    toast({ tone: "danger", text: t("link.failed", { error: engineText(error) }) });
  }
}

/** Sitio al que lleva la dirección, para mostrarlo aparte (ayuda a notar un enlace que se hace pasar por otro). */
export function siteOf(url: string): string {
  try {
    const u = new URL(url);
    return u.protocol === "mailto:" ? u.pathname.split("?")[0]! : u.hostname;
  } catch {
    return "";
  }
}
