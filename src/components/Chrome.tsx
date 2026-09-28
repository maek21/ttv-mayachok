import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { api, on, type IslandPayload, type IslandReminders } from "../lib/api";
import { useSettings } from "../lib/settings";
import { Icon, Keys, Logo, type IconName } from "./ui";

export type Page = "home" | "history" | "reminders" | "dictionary" | "settings";

export function TitleBar({ title = "" }: { title?: string }) {
  const w = getCurrentWindow();
  return (
    <div className="titlebar" data-tauri-drag-region>
      {title && <Logo size={18} />}
      {title && <span className="title" data-tauri-drag-region>{title}</span>}
      <div className="grow" data-tauri-drag-region style={{ alignSelf: "stretch" }} />
      <button type="button" className="winbtn" aria-label="Свернуть" onClick={() => w.minimize()}><Icon name="min" size={14} stroke={1.6} /></button>
      <button type="button" className="winbtn" aria-label="Развернуть" onClick={() => w.toggleMaximize()}><Icon name="max" size={14} stroke={1.6} /></button>
      <button type="button" className="winbtn close" aria-label="Закрыть" onClick={() => w.close()}><Icon name="x" size={14} stroke={1.6} /></button>
    </div>
  );
}

const NAV: { id: Page; label: string; icon: IconName }[] = [
  { id: "home", label: "Главная", icon: "home" },
  { id: "history", label: "История", icon: "clock" },
  { id: "reminders", label: "Маячки", icon: "bell" },
  { id: "dictionary", label: "Словарь", icon: "book" },
  { id: "settings", label: "Настройки", icon: "gear" },
];

export function useEngineState() {
  const [state, setState] = useState<IslandPayload | null>(null);
  useEffect(() => {
    api.islandState().then(setState).catch(() => {});
    return on<IslandPayload>("island", setState);
  }, []);
  return state;
}

/** Сколько маячков на острове — бейдж в сайдбаре */
function usePinCount() {
  const [n, setN] = useState(0);
  useEffect(() => {
    api.islandReminders().then((r) => setN(r.pins.length)).catch(() => {});
    return on<IslandReminders>("island-reminders", (r) => setN(r.pins.length));
  }, []);
  return n;
}

export function Sidebar({ page, go }: { page: Page; go: (p: Page) => void }) {
  const { settings } = useSettings();
  const pins = usePinCount();
  const [mic, setMic] = useState<string | null>(null);
  const engine = useEngineState();

  useEffect(() => {
    if (settings.micDevice) setMic(settings.micDevice);
    else api.defaultMic().then(setMic).catch(() => setMic(null));
  }, [settings.micDevice]);

  const paused = settings.pausedUntil != null && settings.pausedUntil > Date.now();
  const recording = engine?.phase === "listening";
  const statusColor = paused ? "var(--warn)" : recording ? "var(--accent)" : mic ? "var(--ok)" : "var(--warn)";
  // «Микрофон (Fifine K669)» → «Fifine K669»
  const micShort = mic?.match(/\(([^)]+)\)\s*$/)?.[1] ?? mic;
  const statusText = paused ? "На паузе" : recording ? "Идёт запись" : mic ? `${micShort} · готов` : "Микрофон не найден";

  return (
    <nav className="sidebar" data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <Logo size={30} />
        <span className="head">Маячок</span>
      </div>
      {NAV.map((n) => (
        <button key={n.id} type="button" className={`nav ${page === n.id ? "active" : ""}`} onClick={() => go(n.id)}>
          <Icon name={n.icon} />
          <span className="grow">{n.label}</span>
          {n.id === "reminders" && pins > 0 && (
            <span className="num" style={{ minWidth: 20, height: 20, padding: "0 6px", boxSizing: "border-box", borderRadius: 10, background: "var(--accent)", color: "#fff", fontSize: 11, fontWeight: 700, display: "inline-flex", alignItems: "center", justifyContent: "center" }}>{pins}</span>
          )}
        </button>
      ))}
      <div className="grow" data-tauri-drag-region />
      <div className="hint-card">
        <span className="cap" style={{ color: "var(--txt2)" }}>
          {settings.keyMode === "hold" ? "Зажми и говори" : "Нажми и говори"}
        </span>
        <Keys keys={settings.hotkeys.dictate} />
        <div className="row cap" style={{ gap: 8 }}>
          <span className="status-dot" style={{ background: statusColor, animation: recording ? "pulse 1.2s infinite" : undefined }} />
          <span className="ellipsis" title={mic ?? undefined}>{statusText}</span>
        </div>
      </div>
    </nav>
  );
}
