import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { SettingsApp } from "./SettingsApp";
import "./index.css";

const windowType = new URLSearchParams(window.location.search).get("window");
const Root = windowType === "settings" ? SettingsApp : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
