import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { ToastApp } from "./ToastApp";
import "./index.css";

const params = new URLSearchParams(window.location.search);
const isToast = params.get("window") === "toast";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isToast ? <ToastApp /> : <App />}
  </React.StrictMode>,
);
