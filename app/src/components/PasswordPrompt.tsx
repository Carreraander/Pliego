import { createEffect, createSignal, Show } from "solid-js";
import { t } from "../i18n";
import { Button, Dialog, Field, TextInput } from "../ui";
import { answerPassword, passwordRequest } from "../state/unlock";

const fileName = (path: string): string => path.split(/[\\/]/).pop() ?? path;

/** Pide la contraseña de un archivo que una herramienta recibe además del documento abierto (ver `state/unlock.ts`). */
export function PasswordPrompt() {
  const [text, setText] = createSignal("");
  createEffect(() => { if (passwordRequest()) setText(""); });
  const req = () => passwordRequest();
  return (
    <Dialog
      open={req() !== null} title={t("password.fileTitle")} onClose={() => answerPassword(null)} onSubmit={() => answerPassword(text())}
      actions={<><Button onClick={() => answerPassword(null)}>{t("password.cancel")}</Button><Button type="submit" variant="primary">{t("password.submit")}</Button></>}
    >
      {/* Solo se dibuja mientras hay una petición: así no hay dos campos «Contraseña» en el DOM. */}
      <Show when={req()}>
        <p>{t("password.fileHint", { name: fileName(req()?.path ?? "") })}</p>
        <Field label={t("password.label")} error={req()?.wrong ? t("password.wrong") : undefined}>
          <TextInput type="password" autofocus value={text()} onInput={(e) => setText(e.currentTarget.value)} />
        </Field>
      </Show>
    </Dialog>
  );
}
