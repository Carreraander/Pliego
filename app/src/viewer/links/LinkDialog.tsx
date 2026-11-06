import { Show } from "solid-js";
import { t } from "../../i18n";
import { Button, Dialog } from "../../ui";
import { cancelOpenUrl, confirmOpenUrl, pendingUrl, siteOf } from "./openUrl";

/** Pregunta antes de abrir una dirección de un documento. Va una sola vez dentro de la ventana (App.tsx). */
export function LinkDialog() {
  return (
    <Dialog
      open={pendingUrl() !== null} title={t("link.openTitle")} onClose={cancelOpenUrl} onSubmit={() => void confirmOpenUrl()}
      actions={<><Button onClick={cancelOpenUrl}>{t("link.openCancel")}</Button><Button type="submit" variant="primary" autofocus>{t("link.openConfirm")}</Button></>}
    >
      <p>{t("link.openBody")}</p>
      <p><code class="pl-link-url" data-testid="link-url">{pendingUrl()}</code></p>
      <Show when={siteOf(pendingUrl() ?? "")}>{(site) => <p><strong>{t("link.openSite", { site: site() })}</strong></p>}</Show>
      <p class="pl-muted">{t("link.openHint")}</p>
    </Dialog>
  );
}
