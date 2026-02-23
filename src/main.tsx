import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, HashRouter } from "react-router-dom";
import { OverlayProvider } from "overlay-kit";
import { QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { AppErrorBoundary } from "./renderer/components/AppErrorBoundary";
import { queryClient } from "./renderer/lib/queryClient";
import "./index.css";
import "./renderer/i18n";

document.documentElement.classList.add("dark");
const isDesktopShell = Boolean(window.__TAURI__?.core?.invoke);
const Router = isDesktopShell ? HashRouter : BrowserRouter;

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <AppErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <OverlayProvider>
          <Router>
            <App />
          </Router>
        </OverlayProvider>
      </QueryClientProvider>
    </AppErrorBoundary>
  </StrictMode>,
)
