import { onMount } from "solid-js";
import { togglePresentation } from "./present";

/** «Presentación» no tiene panel: al elegirla (menú, paleta o F11) entra a pantalla completa y cierra el inspector. */
export default function PresentLauncher(p: { onClose: () => void }) {
  onMount(() => { p.onClose(); void togglePresentation(); });
  return null;
}
