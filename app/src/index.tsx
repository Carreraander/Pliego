/* @refresh reload */
import { render } from "solid-js/web";
import "./styles/tokens.css";
import "./styles/base.css";

const root = document.getElementById("root")!;

if (import.meta.env.DEV && location.pathname === "/ui-catalog") {
  // Catálogo de componentes: solo en desarrollo (W19).
  void import("./ui/Catalog").then(({ Catalog }) => render(() => <Catalog />, root));
} else if (import.meta.env.DEV && location.pathname === "/ui-shell") {
  // Armazón con contenido de ejemplo (W19).
  void import("./shell/Demo").then(({ ShellDemo }) => render(() => <ShellDemo />, root));
} else {
  void Promise.all([import("./App"), import("./styles/viewer.css")]).then(([{ App }]) => render(() => <App />, root));
}
