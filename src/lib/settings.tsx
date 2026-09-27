import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api, on, type Settings } from "./api";

interface Ctx {
  settings: Settings;
  update: (patch: Partial<Settings> | ((s: Settings) => Partial<Settings>)) => void;
}

const SettingsContext = createContext<Ctx | null>(null);

export function SettingsProvider({ children, fallback }: { children: ReactNode; fallback?: ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const latest = useRef<Settings | null>(null);
  const saveTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    api.getSettings().then((s) => {
      latest.current = s;
      setSettings(s);
    });
    return on<Settings>("settings-changed", (s) => {
      latest.current = s;
      setSettings(s);
    });
  }, []);

  const update = useCallback<Ctx["update"]>((patch) => {
    const cur = latest.current;
    if (!cur) return;
    const p = typeof patch === "function" ? patch(cur) : patch;
    const next = { ...cur, ...p };
    latest.current = next;
    setSettings(next);
    // Слайдеры шлют много событий — сохраняем с небольшой задержкой
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      if (latest.current) api.saveSettings(latest.current).catch(console.error);
    }, 150);
  }, []);

  if (!settings) return <>{fallback ?? null}</>;
  return <SettingsContext.Provider value={{ settings, update }}>{children}</SettingsContext.Provider>;
}

export function useSettings() {
  const c = useContext(SettingsContext);
  if (!c) throw new Error("useSettings вне SettingsProvider");
  return c;
}

/* ---------- мелкие форматтеры ---------- */

export function plural(n: number, one: string, few: string, many: string) {
  const a = Math.abs(n) % 100;
  const b = a % 10;
  if (a > 10 && a < 20) return many;
  if (b > 1 && b < 5) return few;
  if (b === 1) return one;
  return many;
}

export const fmtNum = (n: number) => n.toLocaleString("ru-RU");

export function fmtDuration(ms: number) {
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export function fmtTime(iso: string) {
  return new Date(iso).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
}

export function dayLabel(iso: string) {
  const d = new Date(iso);
  const today = new Date();
  const y = new Date();
  y.setDate(today.getDate() - 1);
  const same = (a: Date, b: Date) => a.toDateString() === b.toDateString();
  if (same(d, today)) return "Сегодня";
  if (same(d, y)) return "Вчера";
  return d.toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: d.getFullYear() === today.getFullYear() ? undefined : "numeric" });
}

export function greeting() {
  const h = new Date().getHours();
  if (h < 5) return "Доброй ночи";
  if (h < 12) return "Доброе утро";
  if (h < 18) return "Добрый день";
  return "Добрый вечер";
}
