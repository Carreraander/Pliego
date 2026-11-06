/** ¿Parece español? Acentos propios del idioma, o al menos dos palabras vacías típicas. Sirve a las pruebas, no al producto. */
const STOP = new Set(("el la los las del que no se un una con por para archivo archivos página páginas documento falta debe puede nombre campo " +
  "texto es en sin ya más está hay al su sus tiene usa indica elige no pudo hace hacen según también este esta estos estas como cuando donde " +
  "algún ningún cada todas todos desde hasta entre sobre dentro fuera nueva nuevo original").split(" "));

// Palabras que solo son españolas: sirven para decidir si una traducción al inglés se ha quedado a medias (sin «no», «es»,
// «en»… que también aparecen en frases inglesas como «match(es)» o «can no longer»).
const STRICT = new Set(("el la los las del que con por para archivo archivos página páginas documento falta debe puede nombre campo texto " +
  "sin más está hay tiene usa indica elige según también este esta estos estas como cuando donde algún ningún cada todas todos " +
  "desde hasta entre sobre dentro fuera nueva nuevo").split(" "));

/** ¿Sigue habiendo español en un texto ya traducido al inglés? Acentos propios del idioma, o dos palabras inequívocas. */
export function stillSpanish(text: string): boolean {
  if (/[áéíóúñ¿¡«»Ñ]/i.test(text)) return true;
  const words = text.toLowerCase().match(/[a-záéíóúñ]+/g) ?? [];
  return words.filter((w) => STRICT.has(w)).length >= 2;
}

export function looksSpanish(text: string): boolean {
  if (/[áéíóúñ¿¡«»Ñ]/i.test(text)) return true;
  const words = text.toLowerCase().match(/[a-záéíóúñ]+/g) ?? [];
  return words.filter((w) => STOP.has(w)).length >= 2;
}
