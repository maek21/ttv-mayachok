import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Icon, Keys, Segmented, Select, Toggle } from "../components/ui";
import { api, on, type Reminder, type ReminderAction, type ReminderPreview, type Repeat } from "../lib/api";
import { dayGroup, dueMs, fmtDue, fmtLeft, pinOrder, REPEAT_LABEL, Ring, ringPct, urgency, useNow } from "../lib/remind";
import { useSettings } from "../lib/settings";

export function useReminders(done: boolean) {
  const [items, setItems] = useState<Reminder[] | null>(null);
  useEffect(() => {
    let alive = true;
    const load = () => api.remindersList(done).then((l) => alive && setItems(l)).catch(() => {});
    load();
    const un = on("reminders-changed", load);
    return () => { alive = false; un(); };
  }, [done]);
  return items;
}

const act = (id: number, a: ReminderAction) => api.reminderAction(id, a).catch(console.error);

function QuickAdd() {
  const { settings } = useSettings();
  const [text, setText] = useState("");
  const [preview, setPreview] = useState<ReminderPreview | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setErr(null);
    if (!text.trim()) return setPreview(null);
    const t = window.setTimeout(() => api.reminderPreview(text).then(setPreview).catch(() => setPreview(null)), 180);
    return () => window.clearTimeout(t);
  }, [text]);

  const add = () => {
    if (!text.trim()) return;
    api.reminderCreate(text).then(() => { setText(""); setPreview(null); }).catch((e) => setErr(String(e)));
  };

  return (
    <div className="col" style={{ gap: 6 }}>
      <label className="row" style={{ height: 46, boxSizing: "border-box", padding: "0 8px 0 16px", borderRadius: 14, background: "var(--surf)", border: "1px solid var(--border)", gap: 10, color: "var(--txt3)" }}>
        <Icon name="plus" size={16} />
        <input value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder="через 20 минут выключить духовку" aria-label="Новый маячок"
          style={{ flexGrow: 1, border: 0, background: "transparent", color: "var(--txt)", fontSize: 14, outline: "none" }} />
        <span className="cap row" style={{ gap: 6 }}>или <Keys keys={settings.hotkeys.dictate} /> «напомни…»</span>
        <button type="button" aria-label="Добавить" onClick={add} disabled={!preview}
          style={{ width: 32, height: 32, border: 0, borderRadius: 10, background: preview ? "var(--accent)" : "var(--surf2)", color: "#fff", display: "flex", alignItems: "center", justifyContent: "center", padding: 0 }}>
          <Icon name="arrowr" size={16} />
        </button>
      </label>
      <div className="cap row" style={{ minHeight: 18, gap: 8, paddingLeft: 16 }}>
        {err ? <span style={{ color: "var(--warn)" }}>{err}</span> : preview ? (
          <>
            <span style={{ color: "var(--txt2)" }}>{fmtDue(preview.dueAt)}</span>·<span>{preview.text}</span>
            {preview.urgent && <span style={{ color: "var(--accent)" }}>· на острове</span>}
            {preview.repeat !== "none" && <span>· {REPEAT_LABEL[preview.repeat].toLowerCase()}</span>}
            <span>· Enter</span>
          </>
        ) : text ? "Не понял, когда — добавь «через 10 минут», «завтра в 9»…" : null}
      </div>
    </div>
  );
}

function UrgentCard({ r, now, selected, onClick }: { r: Reminder; now: number; selected: boolean; onClick: () => void }) {
  const u = urgency(r, now);
  const left = dueMs(r) - now;
  const hot = u !== "calm";
  const color = u === "over" ? "var(--warn)" : "var(--accent)";
  const num = left <= 0 ? "!" : left < 3600000 ? String(Math.ceil(left / 60000)) : `${Math.floor(left / 3600000)}ч`;
  return (
    <button type="button" onClick={onClick} className="row" style={{
      flex: "1 1 0", minWidth: 0, boxSizing: "border-box", padding: "14px 16px", borderRadius: 18, background: "var(--surf)", gap: 14, textAlign: "left",
      border: selected ? "2px solid var(--accent)" : hot ? `1px solid ${color}` : "1px solid var(--border)",
    }}>
      <Ring pct={u === "over" ? 1 : ringPct(r, now)} size={52} stroke={5} color={color} track="var(--surf2)">
        <span className="num" style={{ fontSize: 13, fontWeight: 700 }}>{num}</span>
      </Ring>
      <span className="col grow" style={{ gap: 3 }}>
        <span className="ellipsis" style={{ fontSize: 16, fontWeight: 600 }}>{r.text}</span>
        <span style={{ fontSize: 13, color: "var(--txt2)" }}>{fmtDue(r.dueAt)}</span>
        <span className="num" style={{ fontSize: 12, fontWeight: 600, color: hot ? color : "var(--txt3)" }}>
          {left > 0 ? `через ${fmtLeft(left)}` : fmtLeft(left)}
        </span>
      </span>
      {r.urgent && <span style={{ color: "var(--accent)", display: "flex" }} title="На острове"><Icon name="pin" size={16} /></span>}
    </button>
  );
}

function Row({ r, now, selected, onClick }: { r: Reminder; now: number; selected: boolean; onClick: () => void }) {
  const done = !!r.doneAt;
  const u = urgency(r, now);
  const left = dueMs(r) - now;
  const time = new Date(r.dueAt).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
  return (
    <div className="row" onClick={onClick} style={{
      gap: 14, padding: "10px 14px", borderRadius: 12, cursor: "default",
      background: selected ? "var(--surf2)" : "transparent", boxShadow: selected ? "inset 2px 0 0 var(--accent)" : "none",
    }}>
      <button type="button" aria-label={done ? "Вернуть" : "Готово"} onClick={(e) => { e.stopPropagation(); act(r.id, done ? "undone" : "done"); }}
        style={{ width: 24, height: 24, border: 0, background: "transparent", color: done ? "var(--ok)" : "var(--txt3)", display: "flex", alignItems: "center", justifyContent: "center", padding: 0 }}>
        <Icon name={done ? "checkc" : "circle"} size={20} stroke={1.8} />
      </button>
      <span className="num" style={{ width: 44, flexShrink: 0, fontSize: 13, color: u === "over" && !done ? "var(--warn)" : "var(--txt2)" }}>{time}</span>
      <span className="grow col" style={{ gap: 3 }}>
        <span style={{ fontSize: 14, ...(done ? { color: "var(--txt3)", textDecoration: "line-through" } : {}) }}>{r.text}</span>
        <span className="row" style={{ gap: 12 }}>
          {r.repeat !== "none" && <span className="row cap" style={{ gap: 4 }}><Icon name="repeat" size={13} />{REPEAT_LABEL[r.repeat].toLowerCase()}</span>}
          {r.urgent && !done && <span className="row cap" style={{ gap: 4, color: "var(--accent)" }}><Icon name="pin" size={13} />на острове</span>}
          {!done && <span className="cap num" style={{ color: u === "over" ? "var(--warn)" : undefined }}>{left > 0 ? `через ${fmtLeft(left)}` : fmtLeft(left)}</span>}
          {done && r.doneAt && <span className="cap">выполнено {fmtDue(r.doneAt).toLowerCase()}</span>}
        </span>
      </span>
    </div>
  );
}

function toLocalInputs(iso: string) {
  const d = new Date(iso);
  const p = (n: number) => String(n).padStart(2, "0");
  return { date: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`, time: `${p(d.getHours())}:${p(d.getMinutes())}` };
}

function Detail({ r, now }: { r: Reminder; now: number }) {
  const { settings } = useSettings();
  const [title, setTitle] = useState(r.text);
  const { date, time } = toLocalInputs(r.dueAt);
  useEffect(() => setTitle(r.text), [r.id, r.text]);

  const save = (patch: Partial<Reminder>) => api.reminderUpdate({ ...r, ...patch }).catch(console.error);
  const setWhen = (d: string, t: string) => {
    const [y, m, dd] = d.split("-").map(Number);
    const [hh, mm] = t.split(":").map(Number);
    if ([y, m, dd, hh, mm].some((x) => Number.isNaN(x))) return;
    save({ dueAt: new Date(y, m - 1, dd, hh, mm).toISOString() });
  };
  const left = dueMs(r) - now;
  const small = { height: 32 };

  return (
    <div className="col fade-in" key={r.id} style={{ width: 360, flexShrink: 0, borderLeft: "1px solid var(--border-soft)", boxSizing: "border-box", padding: "18px 22px", gap: 14, overflow: "auto" }}>
      <div className="row" style={{ gap: 8 }}>
        {r.urgent && <span className="row" style={{ gap: 6, height: 24, padding: "0 10px", borderRadius: 12, background: "color-mix(in srgb, var(--accent) 16%, transparent)", color: "var(--accent)", fontSize: 12, fontWeight: 600 }}><Icon name="pin" size={12} />Срочный</span>}
        <div className="grow" />
        {!r.doneAt && <span className="cap num" style={{ color: left <= 0 ? "var(--warn)" : undefined }}>{left > 0 ? `через ${fmtLeft(left)}` : fmtLeft(left)}</span>}
      </div>
      <input value={title} onChange={(e) => setTitle(e.target.value)} onBlur={() => title.trim() && title !== r.text && save({ text: title.trim() })}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} aria-label="Что напомнить"
        style={{ border: 0, background: "transparent", color: "var(--txt)", fontSize: 20, fontWeight: 600, outline: "none", padding: 0 }} />
      <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 10 }}>
        <label className="col" style={{ gap: 6 }}><span className="cap">Дата</span>
          <input type="date" className="input" value={date} onChange={(e) => setWhen(e.target.value, time)} /></label>
        <label className="col" style={{ gap: 6 }}><span className="cap">Время</span>
          <input type="time" className="input" value={time} onChange={(e) => setWhen(date, e.target.value)} /></label>
      </div>
      <div className="col" style={{ gap: 0 }}>
        {[
          ["Повтор", <Select key="rep" width={170} value={r.repeat} onChange={(v) => save({ repeat: v as Repeat })}
            options={(Object.keys(REPEAT_LABEL) as Repeat[]).map((k) => ({ value: k, label: REPEAT_LABEL[k] }))} />],
          ["Держать на острове", <Toggle key="urg" checked={r.urgent} onChange={(v) => save({ urgent: v })} />],
          ["Напомнить заранее", <Select key="lead" width={170} value={String(r.leadMin)} onChange={(v) => save({ leadMin: Number(v) })}
            options={[{ value: "-1", label: `По умолчанию · ${settings.reminders.leadMin} мин` }, { value: "0", label: "Не надо" },
              { value: "5", label: "За 5 минут" }, { value: "10", label: "За 10 минут" }, { value: "30", label: "За 30 минут" }, { value: "60", label: "За час" }]} />],
          ["Уведомление Windows", <Toggle key="toast" checked={r.toast} onChange={(v) => save({ toast: v })} />],
        ].map(([label, ctl], i) => (
          <div key={i} className="row" style={{ gap: 12, padding: "10px 0", borderTop: "1px solid var(--border)", ...small }}>
            <span className="grow" style={{ fontSize: 13 }}>{label}</span>{ctl}
          </div>
        ))}
      </div>
      {r.phrase && (
        <div className="card col" style={{ padding: 12, gap: 6 }}>
          <span className="cap">Сказано голосом · {fmtDue(r.createdAt).toLowerCase()}{r.app ? ` · ${r.app}` : ""}</span>
          <span style={{ fontSize: 13, color: "var(--txt2)", fontStyle: "italic", userSelect: "text" }}>«{r.phrase}»</span>
        </div>
      )}
      <div className="grow" />
      <div className="row" style={{ gap: 8 }}>
        {r.doneAt
          ? <Button kind="subtle" size="sm" icon="repeat" onClick={() => act(r.id, "undone")}>Вернуть</Button>
          : <>
              <Button kind="primary" size="sm" icon="check" onClick={() => act(r.id, "done")}>Готово</Button>
              <Button kind="subtle" size="sm" onClick={() => act(r.id, "snooze")}>+{settings.reminders.snoozeMin} мин</Button>
            </>}
        <div className="grow" />
        <button type="button" className="iconbtn" aria-label="Удалить" onClick={() => act(r.id, "delete")}><Icon name="trash" size={16} /></button>
      </div>
    </div>
  );
}

function Example({ phrase, result, tag }: { phrase: string; result: string; tag?: string }) {
  return (
    <div className="row" style={{ gap: 16, padding: "14px 18px", borderTop: "1px solid var(--border)" }}>
      <span style={{ flex: "1.2 1 0", fontSize: 15 }}>«{phrase}»</span>
      <span className="faint" style={{ display: "flex" }}><Icon name="arrowr" size={16} /></span>
      <span className="row" style={{ flex: "1 1 0", gap: 8, fontSize: 13, color: "var(--txt2)" }}>
        {result}{tag && <span style={{ height: 22, padding: "0 8px", borderRadius: 11, background: "var(--surf2)", fontSize: 11, display: "inline-flex", alignItems: "center" }}>{tag}</span>}
      </span>
    </div>
  );
}

function Empty() {
  const { settings } = useSettings();
  return (
    <div className="col fade-in" style={{ gap: 22 }}>
      <div className="row" style={{ gap: 22, padding: "8px 0 4px" }}>
        <span style={{ width: 72, height: 72, borderRadius: 36, background: "#000", border: "1px solid #34343a", color: "var(--accent)", display: "flex", alignItems: "center", justifyContent: "center", boxShadow: "0 0 40px color-mix(in srgb, var(--accent) 25%, transparent)", flexShrink: 0 }}>
          <Icon name="bell" size={32} stroke={1.7} />
        </span>
        <div className="col" style={{ gap: 6 }}>
          <h2 style={{ fontSize: 24 }}>Пока ни одного маячка</h2>
          <p style={{ margin: 0, fontSize: 15, lineHeight: 1.6, color: "var(--txt2)" }}>
            Зажми <Keys keys={settings.hotkeys.dictate} /> где угодно и начни фразу с «{settings.reminders.triggers.join("», «")}» — текст не вставится, а станет напоминанием.
          </p>
        </div>
      </div>
      <div className="card" style={{ overflow: "hidden" }}>
        <div className="row" style={{ padding: "14px 18px", justifyContent: "space-between" }}><h2 style={{ fontSize: 16 }}>Так можно говорить</h2><span className="cap">время понимаю словами и цифрами</span></div>
        <Example phrase="напомни через 20 минут выключить духовку" result="через 20 минут" />
        <Example phrase="маячок, завтра в семь вечера созвон по билду" result="Завтра, 19:00" />
        <Example phrase="срочно: в пол четвёртого отправить отчёт" result="Сегодня, 15:30" tag="на острове" />
        <Example phrase="каждый понедельник в 10 планёрка" result="Пн, 10:00" tag="повтор" />
        <Example phrase="напомни в пятницу купить подарок" result={`Пт, ${settings.reminders.defaultHour}:00`} tag="утро по умолчанию" />
      </div>
    </div>
  );
}

export function Reminders() {
  const [tab, setTab] = useState<"active" | "done">("active");
  const items = useReminders(tab === "done");
  const now = useNow(1000);
  const [selId, setSelId] = useState<number | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!items) return;
    if (selId == null || !items.some((r) => r.id === selId)) setSelId(items[0]?.id ?? null);
  }, [items, selId]);

  const onIsland = useMemo(() => pinOrder((items ?? []).filter((r) => !r.doneAt && (r.urgent || r.fired || dueMs(r) - now < 3600000)), now), [items, now]);
  const urgent = onIsland.slice(0, 2);
  const groups = useMemo(() => {
    const out: { label: string; items: Reminder[] }[] = [];
    for (const r of items ?? []) {
      const label = tab === "done" ? "ВЫПОЛНЕНО" : dayGroup(r.dueAt);
      const g = out.find((x) => x.label === label);
      if (g) g.items.push(r); else out.push({ label, items: [r] });
    }
    return out;
  }, [items, tab]);

  const selected = items?.find((r) => r.id === selId) ?? null;
  const empty = items !== null && items.length === 0;

  return (
    <div className="grow" style={{ display: "flex", minHeight: 0 }}>
      <div className="grow col" style={{ boxSizing: "border-box", padding: "12px 24px 0 32px", gap: 14, minHeight: 0 }}>
        <div className="page-head">
          <div className="col" style={{ gap: 4 }}>
            <h1>Маячки</h1>
            <span className="muted">Скажи «напомни…» — и Маячок не даст забыть.</span>
          </div>
          <Segmented value={tab} onChange={setTab} options={[{ value: "active", label: "Активные" }, { value: "done", label: "Выполненные" }]} />
        </div>
        {tab === "active" && <QuickAdd />}
        {empty && tab === "active" ? <Empty /> : empty ? <div className="empty cap">Здесь будут выполненные маячки</div> : (
          <div ref={listRef} className="col" style={{ gap: 2, overflow: "auto", minHeight: 0, paddingBottom: 24 }}>
            {tab === "active" && urgent.length > 0 && (
              <div className="col" style={{ gap: 8, paddingBottom: 6 }}>
                <span style={{ padding: "0 4px", fontSize: 12, fontWeight: 600, letterSpacing: ".04em", color: "var(--txt3)" }}>НА ОСТРОВЕ · {onIsland.length}</span>
                <div className="row" style={{ gap: 12, alignItems: "stretch" }}>
                  {urgent.map((r) => <UrgentCard key={r.id} r={r} now={now} selected={r.id === selId} onClick={() => setSelId(r.id)} />)}
                </div>
              </div>
            )}
            {groups.map((g) => (
              <div key={g.label} className="col" style={{ gap: 2 }}>
                <div style={{ padding: "14px 14px 4px", fontSize: 12, fontWeight: 600, letterSpacing: ".04em", color: g.label === "ПРОСРОЧЕНО" ? "var(--warn)" : "var(--txt3)" }}>{g.label} · {g.items.length}</div>
                {g.items.map((r) => <Row key={r.id} r={r} now={now} selected={r.id === selId} onClick={() => setSelId(r.id)} />)}
              </div>
            ))}
          </div>
        )}
      </div>
      {selected && <Detail r={selected} now={now} />}
    </div>
  );
}
