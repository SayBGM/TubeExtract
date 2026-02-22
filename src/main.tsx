import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, HashRouter } from "react-router-dom";
import { OverlayProvider } from "overlay-kit";
import App from "./App";
import "./index.css";
import "./renderer/i18n";

document.documentElement.classList.add("dark");
const Router = window.electronAPI ? HashRouter : BrowserRouter;

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <OverlayProvider>
      <Router>
        <App />
      </Router>
    </OverlayProvider>
  </StrictMode>,
)
