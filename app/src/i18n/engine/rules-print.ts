import type { Rule } from "./rules-system";

/** Mensajes de Imprimir (`pliego-printer`, `pliego_pdf::print` y el servicio de la app). [español, inglés] */
export const PRINT_RULES: readonly Rule[] = [
  ["PDFium no pudo abrir el documento para imprimir (código {})", "PDFium could not open the document for printing (code {})"],
  ["PDFium no pudo cargar la página {}", "PDFium could not load page {}"],
  ["las copias van de 1 a 999", "copies go from 1 to 999"],
  ["la escala va del 10 % al 400 %", "the scale goes from 10% to 400%"],
  ["páginas no válidas: «{}»", "invalid pages: “{}”"],
  ["no hay ninguna página que imprimir con esa selección", "there is no page to print with that selection"],
  ["imprimir solo está disponible en Windows", "printing is only available on Windows"],
  ["el autor del documento no permite imprimirlo (permisos del PDF)", "the document's author does not allow printing it (PDF permissions)"],
  ["la impresora «{}» no responde", "the printer “{}” is not responding"],
  ["no se pudo abrir la impresora «{}»", "could not open the printer “{}”"],
  ["la impresora «{}» no da sus ajustes", "the printer “{}” does not report its settings"],
  ["no se pudo consultar la impresora «{}»", "could not query the printer “{}”"],
  ["la impresora no aceptó el documento (comprueba que está encendida y sin errores)", "the printer did not accept the document (check that it is on and has no errors)"],
  ["la impresora no aceptó la hoja", "the printer did not accept the sheet"],
  ["la impresora rechazó la hoja al terminarla", "the printer rejected the sheet when it was finished"],
  ["la impresora no pudo terminar el documento", "the printer could not finish the document"],
  ["no hay ninguna impresora instalada: añade una en la configuración de Windows", "no printer is installed: add one in the Windows settings"],
  ["respuesta de la impresión no válida: {}", "invalid print response: {}"],
  ["respuesta de la impresora no válida: {}", "invalid printer response: {}"],
  ["Hoja {} × {} pt, zona imprimible {} × {} pt (margen izquierdo {}, superior {})", "Sheet {1} × {2} pt, printable area {3} × {4} pt (left margin {5}, top {6})"],
  ["{}: color {}, doble cara {}, hasta {} copias{}", "{1}: color {2}, two-sided {3}, up to {4} copies{5}"],
];
