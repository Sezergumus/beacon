import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@fontsource-variable/nunito";
import "./index.css";
import { disableBrowserBehavior } from "./lib/disableBrowserBehavior";

disableBrowserBehavior();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
