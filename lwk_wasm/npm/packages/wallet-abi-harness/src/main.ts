import "./styles.css";

import { mountApp } from "./app.js";

const root = document.getElementById("app");
if (!(root instanceof HTMLElement)) {
  throw new Error("Missing #app root node.");
}

void mountApp(root);
