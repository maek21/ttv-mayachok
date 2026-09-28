import { useEffect, useState } from "react";
import type { Reminder, Repeat } from "./api";

/** Текущее время, тикает раз в `ms` */
export function useNow(ms = 1000) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), ms);
    return () => window.clearInterval(t);
  }, [ms]);
  return now;
}

export const dueMs = (r: Pick<Reminder, "dueAt">) => new Date(r.dueAt).getTime();

/** «4:32» в последние 5 минут, «12 мин», «3 ч 40 мин», «2 дн» */
export function fmtLeft(ms: number, short = false) {
  if (ms <= 0) {
    const over = Math.round(-ms / 60000);
    return over < 1 ? "сейчас" : `просрочен на ${fmtSpan(over * 60000)}`;
  }
  if (ms < 5 * 60000) {
    const s = Math.ceil(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }
  return short ? fmtSpanShort(ms) : fmtSpan(ms);
}

function fmtSpan(ms: number) {
  const m = Math.round(ms / 60000);
  if (m < 60) return `${m} мин`;
  const h = Math.floor(m / 60);
  if (h < 24) return m % 60 ? `${h} ч ${m % 60} мин` : `${h} ч`;
  const d = Math.floor(h / 24);
  return h % 24 ? `${d} дн ${h % 24} ч` : `${d} дн`;
}

function fmtSpanShort(ms: number) {
  const m = Math.round(ms / 60000);
  if (m < 60) return `${m}м`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}ч`;
  return `${Math.floor(h / 24)}д`;
}

const WD = ["вс", "пн", "вт", "ср", "чт", "пт", "сб"];

/** «Сегодня, 15:30» · «Завтра, 19:00» · «Пт, 2 окт, 9:00» */
export function fmtDue(iso: string) {
  const d = new Date(iso);
  const now = new Date();
  const time = d.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(d) - day(now)) / 86400000);
  if (diff === 0) return `Сегодня, ${time}`;
  if (diff === 1) return `Завтра, ${time}`;
  if (diff === -1) return `Вчера, ${time}`;
  const wd = WD[d.getDay()];
  const date = d.toLocaleDateString("ru-RU", { day: "numeric", month: "short" }).replace(".", "");
  return `${wd[0].toUpperCase() + wd.slice(1)}, ${date}, ${time}`;
}

export function dayGroup(iso: string) {
  const d = new Date(iso);
  const now = new Date();
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(d) - day(now)) / 86400000);
  if (diff < 0) return "ПРОСРОЧЕНО";
  if (diff === 0) return "СЕГОДНЯ";
  if (diff === 1) return "ЗАВТРА";
  return "ПОЗЖЕ";
}

export const REPEAT_LABEL: Record<Repeat, string> = {
  none: "Не повторять",
  daily: "Каждый день",
  weekdays: "По будням",
  weekly: "Каждую неделю",
  monthly: "Каждый месяц",
};

/** Доля «сколько осталось» для кольца: последний час убывает от полного к пустому */
export function ringPct(r: Reminder, now: number) {
  const left = dueMs(r) - now;
  if (left <= 0) return 0;
  const created = new Date(r.createdAt).getTime() || now;
  const span = Math.min(Math.max(dueMs(r) - created, 60000), 3600000);
  return Math.max(0.04, Math.min(1, left / span));
}

export function Ring({ pct, size = 26, stroke = 3, color = "var(--accent)", track = "#2c2c31", children }: {
  pct: number; size?: number; stroke?: number; color?: string; track?: string; children?: React.ReactNode;
}) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  return (
    <span style={{ position: "relative", width: size, height: size, flexShrink: 0, display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} style={{ position: "absolute", inset: 0, transform: "rotate(-90deg)" }} aria-hidden="true">
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={track} strokeWidth={stroke} />
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={color} strokeWidth={stroke} strokeLinecap="round"
          strokeDasharray={c} strokeDashoffset={c * (1 - pct)} style={{ transition: "stroke-dashoffset 1s linear, stroke .3s" }} />
      </svg>
      {children}
    </span>
  );
}

/** Порядок на острове: «меньше 5 минут» → просроченные → остальные по сроку */
export function pinOrder<T extends Reminder>(pins: T[], now: number): T[] {
  const rank = (r: T) => ({ soon: 0, over: 1, calm: 2 })[urgency(r, now)];
  return [...pins].sort((a, b) => rank(a) - rank(b) || dueMs(a) - dueMs(b));
}

/** Состояние маячка для цвета: обычный, скоро (<5 мин), просрочен */
export function urgency(r: Reminder, now: number): "calm" | "soon" | "over" {
  const left = dueMs(r) - now;
  if (left <= 0 || r.fired) return "over";
  if (left < 5 * 60000) return "soon";
  return "calm";
}
