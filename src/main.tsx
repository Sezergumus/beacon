import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@fontsource-variable/nunito";
import "./index.css";
import { disableBrowserBehavior } from "./lib/disableBrowserBehavior";
import { installGlobalErrorHandlers } from "./lib/globalErrors";
import { ErrorBoundary } from "./components/ErrorBoundary";

disableBrowserBehavior();
installGlobalErrorHandlers();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary variant="app">
      <App />
    </ErrorBoundary>
  </React.StrictMode>
);
