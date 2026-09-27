import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { IslandApp } from "./island/IslandApp";
import { SettingsProvider } from "./lib/settings";
import "./styles.css";

const isIsland = window.location.hash.startsWith("#/island");

// Контекстное меню браузера в десктопном приложении ни к чему (кроме полей ввода)
window.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!t.closest("input, textarea")) e.preventDefault();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SettingsProvider>{isIsland ? <IslandApp /> : <App />}</SettingsProvider>
  </StrictMode>,
);
