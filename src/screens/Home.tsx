import { useEffect, useState } from "react";
import { useEngineState, type Page } from "../components/Chrome";
import { Button, Icon } from "../components/ui";
import { api, on, type Entry, type Stats } from "../lib/api";
import { fmtNum, greeting, plural } from "../lib/settings";
import { EntryRow } from "./History";
import { useReminders } from "./Reminders";
import { dueMs, fmtLeft, Ring, ringPct, urgency, useNow } from "../lib/remind";

function Stat({ label, value, sub }: { label: string; value: string; sub: string }) {
  return (
    <div className="card col" style={{ flex: "1 1 0", boxSizing: "border-box", padding: 18, gap: 6, minWidth: 0 }}>
      <span style={{ fontSize: 13, color: "var(--txt2)" }}>{label}</span>
      <span className="head num" style={{ fontSize: 34, letterSpacing: "-0.02em" }}>{value}</span>
      <span className="cap ellipsis">{sub}</span>
    </div>
  );
}

function Week({ stats }: { stats: Stats }) {
  const max = Math.max(1, ...stats.week.map((d) => d.words));
  const [hover, setHover] = useState<number | null>(null);
  return (
    <div className="row" style={{ gap: 8, alignItems: "flex-end", position: "relative" }} role="img"
      aria-label={`Слов за 7 дней: ${stats.week.map((d) => `${d.label} ${d.words}`).join(", ")}`}>
      {stats.week.map((d, i) => (
        <div key={i} className="col" style={{ flex: "1 1 0", alignItems: "center", gap: 8, position: "relative" }}
          onMouseEnter={() => setHover(i)} onMouseLeave={() => setHover(null)}>
          {hover === i && (
            <span className="num" style={{ position: "absolute", top: -26, padding: "3px 8px", borderRadius: 8, background: "var(--inverse-bg)", color: "var(--inverse-txt)", fontSize: 12, whiteSpace: "nowrap" }}>
              {fmtNum(d.words)} {plural(d.words, "слово", "слова", "слов")}
            </span>
          )}
          <div style={{ width: "100%", height: 110, display: "flex", alignItems: "flex-end" }}>
            <div style={{
              width: "100%", height: Math.max(3, Math.round((d.words / max) * 110)), borderRadius: "4px 4px 0 0",
              background: d.today ? "var(--accent)" : "var(--key-border)", transition: "height .4s",
            }} />
          </div>
          <span style={{ fontSize: 11, color: d.today ? "var(--txt)" : "var(--txt3)" }}>{d.label}</span>
        </div>
      ))}
    </div>
  );
}

export function Home({ go }: { go: (p: Page) => void }) {
  const engine = useEngineState();
  const [stats, setStats] = useState<Stats | null>(null);
  const [recent, setRecent] = useState<Entry[]>([]);

  useEffect(() => {
    const load = () => {
      api.stats().then(setStats).catch(() => {});
      api.historyList("", false, null, 4, 0).then(setRecent).catch(() => {});
    };
    load();
    return on("history-changed", load);
  }, []);

  const recording = engine?.phase === "listening";
  const active = useReminders(false);
  const now = useNow(1000);
  const upcoming = (active ?? []).slice(0, 3);
  const s = stats;
  const vsAvg = s && s.wordsAvg > 0 ? Math.round(((s.wordsToday - s.wordsAvg) / s.wordsAvg) * 100) : null;
  const tagline = !s || s.totalCount === 0
    ? "Зажми сочетание в любом окне и скажи первую фразу."
    : s.savedMinutesToday > 0
      ? `Сегодня голос сэкономил тебе ~${s.savedMinutesToday} ${plural(s.savedMinutesToday, "минуту", "минуты", "минут")} набора.`
      : "Продолжай в том же духе.";

  return (
    <div className="page">
      <div className="page-head">
        <div className="col" style={{ gap: 6 }}>
          <h1 style={{ fontSize: 34 }}>{greeting()}</h1>
          <span className="muted">{tagline}</span>
        </div>
        <Button kind={recording ? "primary" : "accent"} icon={recording ? "stop" : "mic"} onClick={() => api.dictationToggle()}>
          {recording ? "Стоп" : "Надиктовать"}
        </Button>
      </div>

      <div className="row" style={{ gap: 16, alignItems: "stretch" }}>
        <Stat label="Слов сегодня" value={s ? fmtNum(s.wordsToday) : "—"}
          sub={vsAvg == null ? "первый день — всё впереди" : `${vsAvg >= 0 ? "+" : ""}${vsAvg}% к среднему`} />
        <Stat label="Время диктовки" value={s ? `${Math.round(s.durationTodayMs / 60000)} мин` : "—"}
          sub={s ? `${s.countToday} ${plural(s.countToday, "запись", "записи", "записей")}` : ""} />
        <Stat label="Сэкономлено" value={s ? `~${s.savedMinutesToday} мин` : "—"} sub="против набора 40 сл./мин" />
        <Stat label="Серия" value={s ? `${s.streak} ${plural(s.streak, "день", "дня", "дней")}` : "—"} sub={s ? `рекорд — ${s.bestStreak}` : ""} />
      </div>

      <div className="row" style={{ gap: 16, alignItems: "stretch", flexGrow: 1, minHeight: 0 }}>
        <div className="card col" style={{ flex: "3 1 0", boxSizing: "border-box", padding: "18px 18px 10px", gap: 6, minWidth: 0, overflow: "hidden" }}>
          <div className="row" style={{ justifyContent: "space-between", padding: "0 4px 6px" }}>
            <h2>Последние</h2>
            <button type="button" className="btn link" style={{ height: 24, color: "var(--txt2)" }} onClick={() => go("history")}>
              Вся история <Icon name="chev" size={14} />
            </button>
          </div>
          {recent.length === 0 ? (
            <div className="empty cap">Пока пусто — продиктуй что-нибудь</div>
          ) : recent.map((e) => <EntryRow key={e.id} e={e} compact onClick={() => go("history")} />)}
        </div>
        <div className="col" style={{ flex: "2 1 0", gap: 16, minWidth: 0 }}>
          <div className="card col" style={{ boxSizing: "border-box", padding: 18, gap: 14 }}>
            <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline" }}>
              <h2 style={{ fontSize: 16 }}>Слов за неделю</h2>
              <span className="cap num">{s ? fmtNum(s.weekTotal) : 0} всего</span>
            </div>
            {s && <Week stats={s} />}
          </div>
          {upcoming.length > 0 ? (
            <div className="card col" style={{ boxSizing: "border-box", padding: "16px 18px 10px", gap: 4 }}>
              <div className="row" style={{ justifyContent: "space-between", paddingBottom: 6 }}>
                <h2 style={{ fontSize: 16 }}>Ближайшие маячки</h2>
                <button type="button" className="btn link" style={{ height: 22, padding: 0, color: "var(--txt2)" }} onClick={() => go("reminders")}>Все <Icon name="chev" size={14} /></button>
              </div>
              {upcoming.map((r) => (
                <div key={r.id} className="row" style={{ gap: 12, padding: "6px 0" }}>
                  <Ring pct={ringPct(r, now)} size={24} stroke={3} color={urgency(r, now) === "over" ? "var(--warn)" : "var(--accent)"} track="var(--surf2)" />
                  <span className="grow ellipsis" style={{ fontSize: 14 }}>{r.text}</span>
                  <span className="cap num" style={{ color: urgency(r, now) === "calm" ? undefined : "var(--accent)" }}>{dueMs(r) > now ? fmtLeft(dueMs(r) - now) : "пора"}</span>
                </div>
              ))}
            </div>
          ) : (
            <div className="card row" style={{ boxSizing: "border-box", padding: "16px 18px", alignItems: "flex-start" }}>
              <span style={{ color: "var(--accent)", display: "flex", paddingTop: 2 }}><Icon name="bell" /></span>
              <span style={{ fontSize: 13, lineHeight: 1.5, color: "var(--txt2)" }}>
                Скажи «напомни через 20 минут выключить духовку» — появится маячок с отсчётом на острове.
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

