import { addToolStrings, toolText } from "../strings";
import { locale } from "../../i18n";

addToolStrings({
  es: {
    "ocr.suffix": "OCR",
    "ocr.recognize.title": "Reconocer texto (OCR)",
    "ocr.recognize.run": "Reconocer y guardar copia",
    "ocr.clean.title": "Enderezar y limpiar",
    "ocr.clean.run": "Limpiar, reconocer y guardar copia",
    "ocr.clean.hint": "Mejora la imagen antes de reconocer el texto: útil en escaneos torcidos, con ruido o mal iluminados. Las páginas del resultado conservan su aspecto original.",
    "ocr.languages.title": "Idiomas de OCR",
    "ocr.languages.installed": "Instalados", "ocr.languages.available": "Disponibles para descargar",
    "ocr.languages.download": "Descargar", "ocr.languages.none": "No hay ningún idioma instalado.",
    "ocr.languages.hint": "Los modelos se descargan una vez, con huella verificada, y se guardan en tu carpeta de usuario.",
    "ocr.languages.started": "Descargando el idioma… sigue en el centro de tareas", "ocr.languages.job": "Descargar el idioma {name}",
    "ocr.languages.refresh": "Actualizar la lista",
    "ocr.langs": "Idiomas del documento", "ocr.langs.loading": "Buscando idiomas instalados…",
    "ocr.langs.none": "No hay idiomas instalados. Descarga alguno en «Idiomas de OCR».",
    "ocr.langs.failed": "No se pudo usar el OCR: {error}",
    "ocr.pages": "Páginas", "ocr.pages.hint": "Vacío = todas. Ejemplo: 1-3,5", "ocr.pages.invalid": "Escribe un rango como 1-3,5",
    "ocr.force": "Reconocer también las páginas que ya tienen texto",
    "ocr.dpi": "Resolución de reconocimiento (ppp)", "ocr.dpi.hint": "Más alta reconoce mejor el texto pequeño pero tarda más",
    "ocr.deskew": "Enderezar páginas torcidas", "ocr.rotate": "Girar páginas de lado o boca abajo",
    "ocr.binarize": "Corregir iluminación irregular", "ocr.denoise": "Quitar ruido",
    "ocr.note": "El original no se modifica: se guarda «{name}».",
    "ocr.started": "Reconociendo texto… sigue en el centro de tareas", "ocr.job": "OCR de {name}",
    "ocr.noDoc": "Abre un documento escaneado para reconocer su texto.",
  },
  en: {
    "ocr.suffix": "OCR",
    "ocr.recognize.title": "Recognize text (OCR)",
    "ocr.recognize.run": "Recognize and save a copy",
    "ocr.clean.title": "Straighten and clean up",
    "ocr.clean.run": "Clean up, recognize and save a copy",
    "ocr.clean.hint": "Improves the image before recognizing the text: useful for skewed, noisy or badly lit scans. The pages in the result keep their original look.",
    "ocr.languages.title": "OCR languages",
    "ocr.languages.installed": "Installed", "ocr.languages.available": "Available to download",
    "ocr.languages.download": "Download", "ocr.languages.none": "No language is installed.",
    "ocr.languages.hint": "Models are downloaded once, with a verified checksum, and kept in your user folder.",
    "ocr.languages.started": "Downloading the language… follow it in the task center", "ocr.languages.job": "Download the {name} language",
    "ocr.languages.refresh": "Refresh the list",
    "ocr.langs": "Document languages", "ocr.langs.loading": "Looking for installed languages…",
    "ocr.langs.none": "No languages installed. Download one in “OCR languages”.",
    "ocr.langs.failed": "OCR is not available: {error}",
    "ocr.pages": "Pages", "ocr.pages.hint": "Empty = all. Example: 1-3,5", "ocr.pages.invalid": "Enter a range like 1-3,5",
    "ocr.force": "Also recognize pages that already have text",
    "ocr.dpi": "Recognition resolution (dpi)", "ocr.dpi.hint": "Higher reads small text better but takes longer",
    "ocr.deskew": "Straighten skewed pages", "ocr.rotate": "Rotate sideways or upside-down pages",
    "ocr.binarize": "Fix uneven lighting", "ocr.denoise": "Remove noise",
    "ocr.note": "The original is not changed: “{name}” is saved.",
    "ocr.started": "Recognizing text… follow it in the task center", "ocr.job": "OCR of {name}",
    "ocr.noDoc": "Open a scanned document to recognize its text.",
  },
});

/** Textos de OCR con sustitución de `{n}`. */
export function oct(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
