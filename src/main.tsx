import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import App from "./App";


// Disable Right Click globally for native feel
document.addEventListener('contextmenu', event => event.preventDefault());

import { DownloadProvider } from "./context/DownloadContext";
import { ErrorBoundary } from "./components/ErrorBoundary";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <DownloadProvider>
        <App />
      </DownloadProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
