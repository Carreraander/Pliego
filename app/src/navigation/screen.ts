import { createSignal, type Component } from "solid-js";

/** Pantalla especial a pantalla completa en el lienzo (Inicio, Ajustes, Comparar…). `null` = el documento activo. */
export interface ScreenContent { id: string; component: Component<any> } // eslint-disable-line @typescript-eslint/no-explicit-any
const [screen, setScreen] = createSignal<ScreenContent | null>(null);
export const currentScreen = screen;
export const openScreen = (s: ScreenContent): void => { setScreen(s); };
export const closeScreen = (): void => { setScreen(null); };
