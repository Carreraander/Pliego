import { OcrForm } from "./OcrForm";

/** Enderezar y limpiar: mejora la imagen (endereza, gira, ilumina, quita ruido) antes de reconocer el texto. */
export default function CleanPanel(p: { onClose: () => void }) {
  return <OcrForm variant="clean" onClose={p.onClose} />;
}
