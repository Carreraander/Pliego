import { createResource, For } from "solid-js";
import { api, type PageLink } from "../../api";
import { t } from "../../i18n";
import type { OpenDoc } from "../../state/documents";
import { toast } from "../../ui";
import { jumpTo } from "./history";
import { requestOpenUrl } from "./openUrl";

/** Sigue un enlace del documento: salto interno (con historial) o dirección (con confirmación). */
export function followLink(doc: Pick<OpenDoc, "id" | "sizes">, target: PageLink["target"]): void {
  if (target.type === "uri") return requestOpenUrl(target.uri);
  if (target.page < 0 || target.page >= doc.sizes.length) {
    toast({ tone: "warn", text: t("link.badPage") });
    return;
  }
  jumpTo(doc.id, { page: target.page, y: target.y ?? undefined });
}

const label = (l: PageLink) => (l.target.type === "page" ? t("link.goto", { page: l.target.page + 1 }) : t("link.web", { uri: l.target.uri }));

/**
 * Enlaces de una página en el modo Leer: un botón transparente sobre cada uno (se alcanza con Tab, Intro lo sigue y
 * el Narrador lo anuncia como enlace). Los datos salen del worker, en el mismo espacio que el texto.
 */
export function LinksLayer(props: { doc: OpenDoc; index: number; zoom: number }) {
  const [links] = createResource(
    () => ({ id: props.doc.id, revision: props.doc.revision, page: props.index }),
    (k) => api.pageLinks(k.id, k.page).catch(() => [] as PageLink[]),
  );
  return (
    <For each={links() ?? []}>{(l) => (
      <button type="button" class="pl-link" role="link" aria-label={label(l)} title={l.target.type === "uri" ? l.target.uri : label(l)}
        data-pl-link={l.target.type} data-target={l.target.type === "uri" ? l.target.uri : String(l.target.page)} data-y={l.target.type === "page" ? (l.target.y ?? "") : ""}
        style={{ left: `${l.rect[0] * props.zoom}px`, top: `${l.rect[1] * props.zoom}px`, width: `${(l.rect[2] - l.rect[0]) * props.zoom}px`, height: `${(l.rect[3] - l.rect[1]) * props.zoom}px` }}
        onClick={() => followLink(props.doc, l.target)} />
    )}</For>
  );
}
