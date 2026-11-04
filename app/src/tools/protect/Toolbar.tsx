import { Button } from "../../ui";
import { openInspector } from "../../shell";
import { pt } from "./strings";

const openProtect = async () => openInspector((await import("./ProtectPanel")).default);
const openRedact = async () => openInspector((await import("./RedactPanel")).default);
const openSanitize = async () => openInspector((await import("./SanitizePanel")).default);

/** Barra del modo Proteger. */
export default function ProtectToolbar() {
  return (
    <>
      <Button variant="ghost" icon="lock" onClick={() => void openProtect()}>{pt("mode.protect")}</Button>
      <Button variant="ghost" icon="eraser" onClick={() => void openSanitize()}>{pt("mode.sanitize")}</Button>
      <Button variant="ghost" icon="redact" onClick={() => void openRedact()}>{pt("mode.redact")}</Button>
    </>
  );
}
