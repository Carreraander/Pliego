import { jobText } from "../../jobs/translate";
import { locale } from "../index";

/**
 * Texto de un mensaje del motor (error, aviso, línea de un informe) en el idioma de la app.
 *
 * Los crates de Rust escriben sus mensajes en español; aquí se traducen con las reglas de `jobs/translate.ts` y
 * `i18n/engine/rules-*.ts`. En español, o sin regla que lo cubra, el texto queda tal cual.
 */
export function engineText(text: string): string {
  // Los mensajes del motor son breves; un texto largo o de varias líneas (la respuesta de un modelo, por ejemplo) no se toca.
  if (text.length > 400 || text.split("\n").length > 3) return text;
  return jobText(text, locale());
}
