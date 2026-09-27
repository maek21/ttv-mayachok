import { useEffect, useMemo, useRef, useState } from "react";
import { IslandView } from "../components/IslandView";
import { Button, Icon, IconButton, Keys } from "../components/ui";
import { api, on, type Entry } from "../lib/api";
import { useToast } from "../lib/hooks";
import { dayLabel, fmtDuration, fmtTime, useSettings } from "../lib/settings";

const APP_COLORS: Record<string, string> = {
  Telegram: "#2aabee", "VS Code": "#3b8eea", Cursor: "#9aa4ff", Notion: "#e6e6e6", Chrome: "#f2c94c", Edge: "#35c1a8",
  Firefox: "#ff7139", Outlook: "#0f78d4", Word: "#2b7cd3", Excel: "#21a366", Slack: "#e01e5a", Discord: "#5865f2",
  Obsidian: "#a88bfa", Teams: "#7b83eb", "Маячок": "#ff453a",
};
function appColor(app: string) {
  if (APP_COLORS[app]) return APP_COLORS[app];
  let h = 0;
  for (const c of app) h = (h * 31 + c.charCodeAt(0)) % 360;
  return `hsl(${h} 60% 62%)`;
}

export function AppChip({ app }: { app: string }) {
  if (!app) return null;
  return (
    <span className="row" style={{ gap: 6, fontSize: 12, color: "var(--txt2)" }}>
      <span style={{ width: 8, height: 8, borderRadius: 2, background: appColor(app) }} />{app}
    </span>
  );
}

export function EntryRow({ e, selected, compact, onClick }: { e: Entry; selected?: boolean; compact?: boolean; onClick?: () => void }) {
  return (
    <button type="button" onClick={onClick} style={{
      width: "100%", boxSizing: "border-box", padding: "12px 14px", border: 0, borderRadius: 12, textAlign: "left",
      background: selected ? "var(--surf2)" : "transparent", boxShadow: selected ? "inset 2px 0 0 var(--accent)" : "none",
      display: "flex", gap: 16, alignItems: "flex-start", flexShrink: 0,
    }}>
      <span className="num" style={{ width: 40, flexShrink: 0, fontSize: 12, color: "var(--txt3)", paddingTop: 2 }}>{fmtTime(e.createdAt)}</span>
      <span className="grow col" style={{ gap: 6 }}>
        <span className={compact ? "ellipsis" : ""} style={{
          fontSize: 14, lineHeight: 1.45, whiteSpace: compact ? "nowrap" : "pre-line",
          display: compact ? undefined : "-webkit-box", WebkitLineClamp: compact ? undefined : 2, WebkitBoxOrient: "vertical", overflow: "hidden",
        }}>{e.text}</span>
        <span className="row" style={{ gap: 14 }}>
          <AppChip app={e.app} />
          <span className="cap">{fmtDuration(e.durationMs)} · {e.words} сл.</span>
          {e.favorite && <span style={{ color: "var(--warn)", display: "flex" }}><Icon name="star" size={12} /></span>}
        </span>
      </span>
    </button>
  );
}

function AudioPlayer({ id }: { id: number }) {
  const [src, setSrc] = useState<string | null>(null);
  const [peaks, setPeaks] = useState<number[]>([]);
  const [pos, setPos] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [duration, setDuration] = useState(0);
  const audio = useRef<HTMLAudioElement>(null);
  const N = 52;

  useEffect(() => {
    let alive = true;
    setSrc(null); setPeaks([]); setPos(0); setPlaying(false);
    api.historyAudio(id).then(async (url) => {
      if (!alive || !url) return;
      setSrc(url);
      try {
        const buf = await (await fetch(url)).arrayBuffer();
        const ctx = new AudioContext();
        const decoded = await ctx.decodeAudioData(buf);
        const ch = decoded.getChannelData(0);
        const step = Math.max(1, Math.floor(ch.length / N));
        const out: number[] = [];
        for (let i = 0; i < N; i++) {
          let m = 0;
          for (let j = i * step; j < Math.min(ch.length, (i + 1) * step); j++) m = Math.max(m, Math.abs(ch[j]));
          out.push(m);
        }
        const max = Math.max(0.01, ...out);
        if (alive) { setPeaks(out.map((v) => v / max)); setDuration(decoded.duration); }
        ctx.close();
      } catch { /* без волны — не страшно */ }
    });
    return () => { alive = false; };
  }, [id]);

  if (!src) return null;
  const toggle = () => {
    const a = audio.current;
    if (!a) return;
    if (a.paused) a.play(); else a.pause();
  };
  const seek = (i: number) => {
    const a = audio.current;
    if (a && duration) { a.currentTime = (i / N) * duration; setPos(i / N); }
  };
  return (
    <div className="card row" style={{ padding: 12, gap: 12 }}>
      <audio ref={audio} src={src} onPlay={() => setPlaying(true)} onPause={() => setPlaying(false)} onEnded={() => { setPlaying(false); setPos(0); }}
        onTimeUpdate={(e) => duration && setPos(e.currentTarget.currentTime / duration)} />
      <button type="button" aria-label={playing ? "Пауза" : "Воспроизвести"} onClick={toggle} style={{
        width: 36, height: 36, borderRadius: 18, border: 0, background: "var(--inverse-bg)", color: "var(--inverse-txt)",
        display: "flex", alignItems: "center", justifyContent: "center", padding: playing ? 0 : "0 0 0 2px", flexShrink: 0,
      }}><Icon name={playing ? "pause" : "play"} size={14} /></button>
      <div className="grow row" style={{ gap: 2, height: 26 }}>
        {(peaks.length ? peaks : Array(N).fill(0.2)).map((v, i) => (
          <div key={i} onClick={() => seek(i)} style={{
            width: 2, flexShrink: 0, borderRadius: 1, height: Math.max(3, Math.round(v * 24)),
            background: i / N < pos ? "var(--accent)" : "var(--key-border)",
          }} />
        ))}
      </div>
      <span className="cap num">{fmtDuration((duration || 0) * 1000)}</span>
    </div>
  );
}

function Detail({ e, onChange, toast }: { e: Entry; onChange: () => void; toast: (m: string) => void }) {
  return (
    <div className="col fade-in" key={e.id} style={{ width: 380, flexShrink: 0, borderLeft: "1px solid var(--border-soft)", boxSizing: "border-box", padding: "20px 24px", gap: 18, overflow: "auto" }}>
      <div className="row" style={{ justifyContent: "space-between" }}>
        <AppChip app={e.app || "—"} />
        <span className="cap">{dayLabel(e.createdAt)}, {fmtTime(e.createdAt)}</span>
      </div>
      <p style={{ margin: 0, fontSize: 20, lineHeight: 1.45, fontWeight: 500, whiteSpace: "pre-line", userSelect: "text" }}>{e.text}</p>
      {e.hasAudio && <AudioPlayer id={e.id} />}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: "12px 16px", fontSize: 13 }}>
        <div className="col" style={{ gap: 2 }}><span className="cap">Слов</span><span>{e.words}</span></div>
        <div className="col" style={{ gap: 2 }}><span className="cap">Язык</span><span>{e.language || "—"}</span></div>
        <div className="col" style={{ gap: 2 }}><span className="cap">Модель</span><span className="ellipsis">{e.model}</span></div>
        <div className="col" style={{ gap: 2 }}><span className="cap">Обработка</span><span>{(e.processMs / 1000).toFixed(1).replace(".", ",")} с · {e.engine === "cloud" ? "облако" : "локально"}</span></div>
      </div>
      {e.rawText && e.rawText !== e.text && (
        <details style={{ fontSize: 13, color: "var(--txt3)" }}>
          <summary style={{ cursor: "default" }}>Как услышал Whisper</summary>
          <p style={{ margin: "8px 0 0", userSelect: "text" }}>{e.rawText}</p>
        </details>
      )}
      <div className="grow" />
      <div className="row" style={{ gap: 8 }}>
        <Button kind="primary" size="sm" icon="copy" onClick={() => api.copyText(e.text).then(() => toast("Скопировано"))}>Копировать</Button>
        <Button kind="subtle" size="sm" icon="insert" onClick={() => api.reinsertText(e.text)}>Вставить</Button>
        <div className="grow" />
        <IconButton icon="star" label={e.favorite ? "Убрать из избранного" : "В избранное"} on={e.favorite} onClick={() => api.toggleFavorite(e.id).then(onChange)} />
        <IconButton icon="trash" label="Удалить" onClick={() => api.historyDelete(e.id).then(onChange)} />
      </div>
    </div>
  );
}

export function SearchBox({ value, onChange, placeholder, width = 320 }: { value: string; onChange: (v: string) => void; placeholder: string; width?: number }) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === "f") { e.preventDefault(); ref.current?.focus(); }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);
  return (
    <label className="row" style={{ width, height: 38, boxSizing: "border-box", padding: "0 12px", borderRadius: 10, background: "var(--surf)", border: "1px solid var(--border)", gap: 8, color: "var(--txt3)" }}>
      <Icon name="search" size={16} />
      <input ref={ref} value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} aria-label={placeholder}
        style={{ flexGrow: 1, minWidth: 0, border: 0, background: "transparent", color: "var(--txt)", fontSize: 14, outline: "none" }} />
      <span className="key">Ctrl F</span>
    </label>
  );
}

export function History() {
  const { settings } = useSettings();
  const [query, setQuery] = useState("");
  const [fav, setFav] = useState(false);
  const [app, setApp] = useState<string | null>(null);
  const [apps, setApps] = useState<string[]>([]);
  const [items, setItems] = useState<Entry[] | null>(null);
  const [selId, setSelId] = useState<number | null>(null);
  const toast = useToast();

  const load = () => {
    api.historyList(query, fav, app, 500, 0).then((list) => {
      setItems(list);
      setSelId((cur) => (cur != null && list.some((e) => e.id === cur) ? cur : list[0]?.id ?? null));
    });
    api.historyApps().then(setApps);
  };
  useEffect(() => {
    const t = window.setTimeout(load, 150);
    return () => window.clearTimeout(t);
  }, [query, fav, app]);
  const loadRef = useRef(load);
  loadRef.current = load;
  useEffect(() => on("history-changed", () => loadRef.current()), []);

  const groups = useMemo(() => {
    const out: { label: string; items: Entry[] }[] = [];
    for (const e of items ?? []) {
      const label = dayLabel(e.createdAt);
      const last = out[out.length - 1];
      if (last && last.label === label) last.items.push(e);
      else out.push({ label, items: [e] });
    }
    return out;
  }, [items]);

  const selected = items?.find((e) => e.id === selId) ?? null;
  const topApps = apps.slice(0, 3);
  const empty = items !== null && items.length === 0 && !query && !fav && !app;

  if (empty) {
    return (
      <div className="page">
        <div className="page-head"><h1>История</h1></div>
        <div className="empty">
          <IslandView settings={settings} levels={[]} p={{ phase: "idle", mode: null, text: "", elapsedMs: 0, words: 0, message: "", app: "" }} />
          <div className="col" style={{ alignItems: "center", gap: 10 }}>
            <h2 style={{ fontSize: 26 }}>Тут пока тихо</h2>
            <p style={{ margin: 0, maxWidth: 420, fontSize: 15, lineHeight: 1.6, color: "var(--txt2)" }}>
              {settings.keyMode === "hold" ? "Зажми" : "Нажми"} <Keys keys={settings.hotkeys.dictate} /> в любом окне и скажи что-нибудь — запись появится здесь.
            </p>
          </div>
          <Button kind="accent" icon="mic" onClick={() => api.dictationToggle()}>Попробовать сейчас</Button>
        </div>
      </div>
    );
  }

  return (
    <div className="grow" style={{ display: "flex", minHeight: 0 }}>
      <div className="grow col" style={{ boxSizing: "border-box", padding: "12px 24px 0 32px", gap: 16, minHeight: 0 }}>
        <div className="page-head" style={{ alignItems: "center" }}>
          <h1>История</h1>
          <SearchBox value={query} onChange={setQuery} placeholder="Поиск по записям" />
        </div>
        <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
          <button type="button" className={`chip ${!fav && !app ? "on" : ""}`} onClick={() => { setFav(false); setApp(null); }}>Все</button>
          <button type="button" className={`chip ${fav ? "on" : ""}`} onClick={() => setFav(!fav)}>Избранное</button>
          {topApps.map((a) => (
            <button key={a} type="button" className={`chip ${app === a ? "on" : ""}`} onClick={() => setApp(app === a ? null : a)}>{a}</button>
          ))}
          {apps.length > 3 && (
            <select className="chip" aria-label="Другие приложения" value={app && !topApps.includes(app) ? app : ""}
              onChange={(e) => setApp(e.target.value || null)} style={{ appearance: "none", paddingRight: 12 }}>
              <option value="">Ещё приложения ▾</option>
              {apps.slice(3).map((a) => <option key={a} value={a}>{a}</option>)}
            </select>
          )}
        </div>
        <div className="col" style={{ gap: 2, overflow: "auto", minHeight: 0, paddingBottom: 24 }}>
          {items?.length === 0 && <div className="cap" style={{ padding: 14 }}>Ничего не нашлось</div>}
          {groups.map((g) => (
            <div key={g.label} className="col" style={{ gap: 2 }}>
              <div style={{ padding: "14px 14px 4px", fontSize: 12, fontWeight: 600, letterSpacing: "0.04em", color: "var(--txt3)" }}>{g.label.toUpperCase()}</div>
              {g.items.map((e) => <EntryRow key={e.id} e={e} selected={e.id === selId} onClick={() => setSelId(e.id)} />)}
            </div>
          ))}
        </div>
      </div>
      {selected && <Detail e={selected} onChange={load} toast={toast.show} />}
      {toast.msg && <div className="toast">{toast.msg}</div>}
    </div>
  );
}
