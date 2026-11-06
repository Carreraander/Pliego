import { Show } from "solid-js";
import { t } from "../i18n";
import { Button, Dialog } from "../ui";
import { answerSignedEdit, signedEditRequest } from "../state/signedEdit";

const fileName = (path: string): string => path.split(/[\\/]/).pop() ?? path;

/** Avisa de que editar un documento firmado invalida sus firmas (ver `state/signedEdit.ts`). */
export function SignedEditPrompt() {
  const req = () => signedEditRequest();
  return (
    <Dialog
      open={req() !== null} title={t("signed.title")} onClose={() => answerSignedEdit(false)} onSubmit={() => answerSignedEdit(true)}
      actions={<><Button onClick={() => answerSignedEdit(false)}>{t("signed.cancel")}</Button><Button type="submit" variant="primary">{t("signed.confirm")}</Button></>}
    >
      <Show when={req()}>
        <p>{t("signed.body", { name: fileName(req()?.path ?? "") })}</p>
      </Show>
    </Dialog>
  );
}
