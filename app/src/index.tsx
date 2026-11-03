/* @refresh reload */
import { render } from "solid-js/web";
import "./styles/tokens.css";
import "./styles/base.css";

const root = document.getElementById("root")!;

if (import.meta.env.DEV && location.pathname === "/ui-catalog") {
  // Catálogo de componentes: solo en desarrollo (W19).
  void import("./ui/Catalog").then(({ Catalog }) => render(() => <Catalog />, root));
} else {
  void Promise.all([import("./App"), import("./styles/provisional.css")]).then(([{ App }]) => render(() => <App />, root));
}
