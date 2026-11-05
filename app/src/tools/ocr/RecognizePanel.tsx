import { OcrForm } from "./OcrForm";

/** Reconocer texto (OCR): añade una capa de texto buscable a un PDF escaneado y guarda una copia. */
export default function RecognizePanel(p: { onClose: () => void }) {
  return <OcrForm variant="recognize" onClose={p.onClose} />;
}
