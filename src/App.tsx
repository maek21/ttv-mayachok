import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { Sidebar, TitleBar, type Page } from "./components/Chrome";
import { api, on, type Settings as S } from "./lib/api";
import { useSettings } from "./lib/settings";
import { Dictionary } from "./screens/Dictionary";
import { History } from "./screens/History";
import { Home } from "./screens/Home";
import { Onboarding } from "./screens/Onboarding";
import { Settings, type Section } from "./screens/Settings";

/** Тема, акцент и «живой» шрифт — общие для обоих окон */
export function useLook(settings: S, pauseWhenUnfocused: boolean) {
  const [focused, setFocused] = useState(true);

  useEffect(() => {
    if (!pauseWhenUnfocused) return;
    const w = getCurrentWindow();
    w.isFocused().then(setFocused).catch(() => {});
    let un: (() => void) | undefined;
    w.onFocusChanged(({ payload }) => setFocused(payload)).then((u) => (un = u));
    return () => un?.();
  }, [pauseWhenUnfocused]);

  useEffect(() => {
    const root = document.documentElement;
    const apply = () => {
      const dark = settings.theme === "dark" || (settings.theme === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
      root.dataset.theme = dark ? "dark" : "light";
    };
    apply();
    root.style.setProperty("--accent", settings.island.accent);
    root.style.setProperty("--amp", String(settings.liveAmplitude));
    const mq = matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [settings.theme, settings.island.accent, settings.liveAmplitude]);

  if (!settings.liveType) return "off";
  return pauseWhenUnfocused && settings.livePauseUnfocused && !focused ? "paused" : "on";
}

export function App() {
  const { settings } = useSettings();
  const live = useLook(settings, true);
  const [page, setPage] = useState<Page>("home");
  const [section, setSection] = useState<Section>("general");
  const [update, setUpdate] = useState<string | null>(null);

  useEffect(() => on<string>("navigate", (p) => {
    if (p === "settings") setPage("settings");
  }), []);

  useEffect(() => {
    if (!settings.autoUpdate) return;
    api.checkUpdate().then((u) => u.available && setUpdate(u.latest)).catch(() => {});
    // только при запуске
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!settings.onboarded) {
    return <div className="app" data-live={live}><Onboarding /></div>;
  }

  return (
    <div className="app" data-live={live}>
      <div className="window">
        <Sidebar page={page} go={setPage} />
        <main className="main">
          <TitleBar />
          {page === "home" && <Home go={setPage} />}
          {page === "history" && <History />}
          {page === "dictionary" && <Dictionary />}
          {page === "settings" && <Settings section={section} setSection={setSection} />}
        </main>
      </div>
      {update && (
        <button type="button" className="toast" style={{ border: 0, cursor: "default" }}
          onClick={() => { setPage("settings"); setSection("about"); setUpdate(null); }}>
          Доступна версия {update} — подробнее
        </button>
      )}
    </div>
  );
}
