import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ThemeProvider } from "./context/ThemeContext";
import { AppProvider } from "./context/AppContext";
import { ToastProvider } from "./components/ui/Toast";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <AppProvider>
        <ToastProvider>
          <App />
        </ToastProvider>
      </AppProvider>
    </ThemeProvider>
  </React.StrictMode>
);
