/**
 * Textos en español del código del motor que NO se traducen en la interfaz, cada uno con su motivo. Lo habitual es
 * que el texto no lo vea nadie (instrucciones al modelo, expresiones regulares, alias que se escriben en la CLI) o
 * que ya se escriba en el idioma de la app por otra vía (un parámetro `lang`, un texto traducido por la interfaz).
 */
const PROMPT = "instrucción al modelo del asistente: no se muestra; el modelo contesta en el idioma de la pregunta";
const REGEX = "expresión regular de reconocimiento: no se muestra";
const ALIAS = "alias español que se escribe en la CLI o en la acción guardada: no se muestra";
const UI = "nombre que la interfaz traduce ella misma (clave dialog.filter.* en i18n)";
const LOG = "texto de registro de diagnóstico: no se muestra en la interfaz";

export const NOT_TRANSLATED: Record<string, string> = {
  "Pliego {} · {} {} Últimas líneas del registro ({}):": "cabecera del informe de fallo que la persona copia para el equipo de Pliego (va al portapapeles tal cual, junto al registro en español)",
  "Eres el asistente de un lector de PDF y trabajas sin conexión. Responde a la pregunta usando SOLO los fragmentos del documento que se te dan. Cada fragmento empieza con su página, por ejemplo [p. 3]. Cita las páginas en las que te basas con ese mismo formato, [p. N], justo después de cada afirmación. Si los fragmentos no contienen la respuesta, dilo claramente y no inventes nada. Responde en el idioma de la pregunta, de forma breve y precisa.": PROMPT,
  "Eres el asistente de un lector de PDF y trabajas sin conexión. Resume el texto que se te da en el idioma del propio texto. Conserva cifras, fechas y nombres importantes. Indica las páginas de origen con el formato [p. N]. No añadas información que no esté en el texto.": PROMPT,
  "Fragmentos del documento: {}Pregunta: {}": PROMPT,
  "Escribe un resumen del documento en un párrafo corto seguido de 3 a 6 puntos clave.": PROMPT,
  "Resume este tramo en un párrafo corto.": PROMPT,
  "Describe esta imagen en una frase breve y objetiva, adecuada como texto alternativo para accesibilidad.": PROMPT,
  "(?i)\\b(?:pp?\\.|págs?\\.|pags?\\.|páginas?|paginas?|pages?)\\s*([0-9][0-9\\s,;\\-–y]*)": REGEX,
  "(?i)(?:[€$£]\\s?\\d[\\d.,]*\\d|\\b\\d[\\d.,]*\\d?\\s?(?:€|euros?|eur\\b|usd\\b|\\$|dólares|dolares))": REGEX,
  "página": "palabra que se reconoce en los pies de página al exportar a Office (entrada, no salida)",
  "impresión": ALIAS,
  "sin-pérdida": ALIAS,
  "separación": ALIAS,
  "teléfono": ALIAS,
  "Imágenes, libros, texto y Office": UI,
  "Acción": UI,
  "sin nombre": LOG,
};
