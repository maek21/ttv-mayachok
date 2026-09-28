import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Icon, Keys } from "../components/ui";
import { api, type Reminder, type ReminderAction, type Settings } from "../lib/api";
import { dueMs, fmtDue, fmtLeft, pinOrder, Ring, ringPct, urgency } from "../lib/remind";

const act = (id: number, a: ReminderAction) => api.reminderAction(id, a).catch(console.error);
const EASE = "cubic-bezier(.2,.9,.25,1.12)";

/** Чёрная капсула, плавно меняющая размер под содержимое */
export function Morph({ children, radius = 30, bg = "#000", glow, k }: { children: ReactNode; radius?: number; bg?: string; glow?: string; k: string }) {
  const inner = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 120, h: 40 });
  useLayoutEffect(() => {
    const el = inner.current;
    if (!el) return;
    const m = () => setBox({ w: el.offsetWidth, h: el.offsetHeight });
    m();
    const ro = new ResizeObserver(m);
    ro.observe(el);
    return () => ro.disconnect();
  }, [k]);
  return (
    <div style={{
      width: box.w, height: box.h, borderRadius: Math.min(radius, box.h / 2), background: bg, color: "#f5f5f7", overflow: "hidden", position: "relative", flexShrink: 0,
      boxShadow: glow ? `0 0 0 4px ${glow}, 0 18px 50px rgba(0,0,0,.5)` : "0 18px 50px rgba(0,0,0,.5)",
      transition: `width .42s ${EASE}, height .42s ${EASE}, border-radius .42s, background .3s, box-shadow .3s`,
      ["--key-bg" as string]: "#1c1c1e", ["--key-border" as string]: "#3a3a3c", ["--txt2" as string]: "#d1d1d6", ["--txt3" as string]: "#8e8e93",
    }}>
      <div ref={inner} key={k} className="fade-in" style={{ position: "absolute", left: 0, top: 0, width: "max-content" }}>{children}</div>
    </div>
  );
}

const colorOf = (u: ReturnType<typeof urgency>) => (u === "over" ? "#ff9f0a" : "var(--accent)");

function leftLabel(r: Reminder, now: number) {
  const left = dueMs(r) - now;
  if (left <= 0) {
    const m = Math.round(-left / 60000);
    return m < 1 ? "сейчас" : `−${m < 60 ? m + " мин" : Math.floor(m / 60) + " ч"}`;
  }
  return fmtLeft(left);
}

/** Маленький кружок: следующий маячок или «+N» */
function Dot({ r, now, more }: { r?: Reminder; now: number; more?: number }) {
  if (more) {
    return (
      <div style={{ width: 40, height: 40, borderRadius: 20, background: "#000", boxShadow: "0 10px 30px rgba(0,0,0,.45)", display: "flex", alignItems: "center", justifyContent: "center", fontSize: 12, fontWeight: 700, color: "#d1d1d6", flexShrink: 0 }}>
        +{more}
      </div>
    );
  }
  if (!r) return null;
  const u = urgency(r, now);
  const left = dueMs(r) - now;
  const n = left <= 0 ? "!" : left < 3600000 ? String(Math.ceil(left / 60000)) : left < 86400000 ? `${Math.floor(left / 3600000)}ч` : `${Math.floor(left / 86400000)}д`;
  return (
    <div title={r.text} style={{ width: 40, height: 40, borderRadius: 20, background: "#000", boxShadow: "0 10px 30px rgba(0,0,0,.45)", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
      <Ring pct={u === "over" ? 1 : ringPct(r, now)} size={28} stroke={3} color={colorOf(u)}>
        <span className="num" style={{ fontSize: 9, fontWeight: 700, color: "#f5f5f7" }}>{n}</span>
      </Ring>
    </div>
  );
}

/** Кружки рядом с островом во время диктовки */
export function PinDots({ pins: raw, now, max = 2 }: { pins: Reminder[]; now: number; max?: number }) {
  if (!raw.length) return null;
  const pins = pinOrder(raw, now);
  const shown = pins.slice(0, max);
  return (
    <div className="row fade-in" style={{ gap: 6 }}>
      {shown.map((r) => <Dot key={r.id} r={r} now={now} />)}
      {pins.length > max && <Dot now={now} more={pins.length - max} />}
    </div>
  );
}

/** Закреплённые: главная капсула + кружки; при наведении — список */
export function PinStack({ pins: raw, now, expanded }: { pins: Reminder[]; now: number; expanded: boolean }) {
  const pins = pinOrder(raw, now);
  const main = pins[0];
  const u = urgency(main, now);
  const soon = u === "soon";

  if (expanded) {
    return (
      <Morph k="list" radius={26}>
        <div style={{ width: 420, boxSizing: "border-box", padding: "14px 10px 10px", display: "flex", flexDirection: "column", gap: 2 }}>
          <div className="row" style={{ padding: "0 8px 8px", fontSize: 12, fontWeight: 600, letterSpacing: ".06em", color: "#8e8e93" }}>
            <Icon name="bell" size={13} />МАЯЧКИ · {pins.length}
          </div>
          <div className="col" style={{ gap: 2, maxHeight: 250, overflow: "auto" }}>
            {pins.map((r) => {
              const ur = urgency(r, now);
              return (
                <div key={r.id} className="row" style={{ gap: 12, padding: "8px 8px", borderRadius: 14, background: ur === "soon" ? "rgba(255,69,58,.12)" : "transparent" }}>
                  <Ring pct={ur === "over" ? 1 : ringPct(r, now)} size={28} stroke={3} color={colorOf(ur)}>
                    {ur === "over" && <span style={{ fontSize: 12, fontWeight: 800, color: "#ff9f0a" }}>!</span>}
                  </Ring>
                  <div className="col grow" style={{ gap: 1, minWidth: 0 }}>
                    <span className="ellipsis" style={{ fontSize: 14, fontWeight: 600 }}>{r.text}</span>
                    <span className="num" style={{ fontSize: 12, color: ur === "calm" ? "#8e8e93" : colorOf(ur) }}>{fmtDue(r.dueAt)} · {ur === "over" ? `просрочен ${leftLabel(r, now).replace("−", "")}` : leftLabel(r, now)}</span>
                  </div>
                  {ur === "over" && (
                    <button type="button" onClick={() => act(r.id, "snooze")} style={pillBtn("dark")}>+10</button>
                  )}
                  <button type="button" aria-label="Готово" onClick={() => act(r.id, "done")} style={{ ...pillBtn("dark"), width: 32, padding: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
                    <Icon name="check" size={15} stroke={2.4} />
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      </Morph>
    );
  }

  const rest = pins.slice(1, 3);
  return (
    <div className="row" style={{ gap: 6 }}>
      <Morph k={`pin-${main.id}-${u}`} bg={soon ? "var(--accent)" : "#000"} glow={soon ? "color-mix(in srgb, var(--accent) 25%, transparent)" : undefined}>
        <div className="row" style={{ height: 40, padding: "0 16px 0 8px", gap: 10, fontSize: 13, whiteSpace: "nowrap", animation: soon ? "pulse 1.6s ease-in-out infinite" : undefined }}>
          <Ring pct={u === "over" ? 1 : ringPct(main, now)} size={26} stroke={3} color={soon ? "#fff" : colorOf(u)} track={soon ? "rgba(255,255,255,.3)" : "#2c2c31"}>
            {u === "over" && <span style={{ fontSize: 12, fontWeight: 800, color: "#ff9f0a" }}>!</span>}
          </Ring>
          <span className="ellipsis" style={{ fontWeight: 600, maxWidth: 200 }}>{main.text}</span>
          <span className="num" style={{ color: soon ? "#fff" : u === "over" ? "#ff9f0a" : "#a1a1a6", fontWeight: soon ? 600 : 400 }}>
            {u === "over" ? (dueMs(main) - now > -60000 ? "сейчас" : `просрочен ${leftLabel(main, now).replace("−", "")}`) : leftLabel(main, now)}
          </span>
        </div>
      </Morph>
      {rest.map((r) => <Dot key={r.id} r={r} now={now} />)}
      {pins.length > 3 && <Dot now={now} more={pins.length - 3} />}
    </div>
  );
}

function pillBtn(kind: "dark" | "light") {
  return {
    height: 32, padding: "0 12px", border: 0, borderRadius: 16, fontSize: 13, fontWeight: 600, flexShrink: 0,
    background: kind === "light" ? "#f5f5f7" : "#1c1c1e", color: kind === "light" ? "#0b0b0c" : "#f5f5f7",
  } as const;
}

/** «Пора!» — единственное состояние острова с кнопками */
export function FireCard({ r, settings }: { r: Reminder; settings: Settings }) {
  const time = new Date(r.dueAt).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
  return (
    <Morph k={`fire-${r.id}`}>
      <div style={{ width: 500, boxSizing: "border-box", padding: "18px 18px 16px", display: "flex", flexDirection: "column", gap: 14 }}>
        <div className="row" style={{ gap: 14 }}>
          <span style={{ width: 46, height: 46, borderRadius: 23, background: "var(--accent)", color: "#fff", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0, boxShadow: "0 0 24px var(--accent)", animation: "pulse 1.4s ease-in-out infinite" }}>
            <Icon name="bell" size={22} />
          </span>
          <div className="col grow" style={{ gap: 2, minWidth: 0 }}>
            <span style={{ fontSize: 12, fontWeight: 700, letterSpacing: ".06em", color: "var(--accent)" }}>ПОРА</span>
            <span style={{ fontSize: 19, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r.text}</span>
            <span style={{ fontSize: 12, color: "#8e8e93" }}>{time}{r.phrase ? ` · сказано ${fmtDue(r.createdAt).toLowerCase()}` : ""}</span>
          </div>
        </div>
        <div className="row" style={{ gap: 8 }}>
          <button type="button" style={pillBtn("dark")} onClick={() => act(r.id, "snooze")}>+{settings.reminders.snoozeMin} мин</button>
          <button type="button" style={pillBtn("dark")} onClick={() => act(r.id, "hour")}>+1 час</button>
          <button type="button" style={pillBtn("dark")} onClick={() => act(r.id, "tomorrow")}>Завтра</button>
          <div className="grow" />
          <button type="button" style={pillBtn("light")} onClick={() => act(r.id, "done")}>Готово</button>
        </div>
      </div>
    </Morph>
  );
}

/** «Маячок поставлен» — пару секунд после фразы */
export function SetCard({ r, now, settings }: { r: Reminder; now: number; settings: Settings }) {
  return (
    <Morph k={`set-${r.id}`} radius={26}>
      <div className="row" style={{ width: 460, boxSizing: "border-box", padding: "14px 16px", gap: 14 }}>
        <span style={{ width: 40, height: 40, borderRadius: 20, background: "#1c1c1e", color: "var(--accent)", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
          <Icon name={r.urgent ? "pin" : "bell"} size={19} />
        </span>
        <div className="col grow" style={{ gap: 2, minWidth: 0 }}>
          <span style={{ fontSize: 12, color: "#8e8e93" }}>{r.urgent ? "Срочный маячок — на острове" : "Маячок поставлен"}</span>
          <span className="ellipsis" style={{ fontSize: 16, fontWeight: 600 }}>{r.text}</span>
          <span className="num" style={{ fontSize: 12, color: "#d1d1d6" }}>{fmtDue(r.dueAt)} · через {fmtLeft(dueMs(r) - now)}</span>
        </div>
        <span className="row" style={{ gap: 6, fontSize: 12, color: "#8e8e93" }}><Keys keys={settings.hotkeys.cancel} />отмена</span>
      </div>
    </Morph>
  );
}
