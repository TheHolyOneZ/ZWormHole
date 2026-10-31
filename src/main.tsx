import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyPlatformAttributes } from "./lib/platform";
import "./styles/app.css";

applyPlatformAttributes();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
