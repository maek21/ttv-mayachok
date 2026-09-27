import { useState } from "react";
import { Button, Icon, Segmented, Toggle } from "../components/ui";
import type { Rule } from "../lib/api";
import { useSettings } from "../lib/settings";

type Tab = "words" | "replace" | "snippet";

const KIND_LABEL: Record<Rule["kind"], string> = { command: "Команда", replace: "Замена", snippet: "Сниппет" };

function show(to: string) {
  return to.replace(/\n\n/g, "¶ абзац").replace(/\n/g, "↵ перенос строки");
}

function RuleRow({ r, onChange, onDelete }: { r: Rule; onChange: (r: Rule) => void; onDelete: () => void }) {
  return (
    <div className="row" style={{ gap: 16, padding: "12px 16px", borderBottom: "1px solid var(--border)" }}>
      <span style={{ flex: "1 1 0", fontSize: 14 }}>«{r.from}»</span>
      <span className="faint" style={{ display: "flex" }}><Icon name="arrowr" size={16} /></span>
      <span className="ellipsis" style={{ flex: "1.4 1 0", fontSize: 14, color: "var(--txt2)" }}>{show(r.to)}</span>
      <span className="cap" style={{ width: 80 }}>{KIND_LABEL[r.kind]}</span>
      <Toggle checked={r.enabled} onChange={(v) => onChange({ ...r, enabled: v })} label={`Включить «${r.from}»`} />
      <button type="button" aria-label={`Удалить «${r.from}»`} onClick={onDelete} style={{ width: 28, height: 28, border: 0, borderRadius: 14, background: "transparent", color: "var(--txt3)", display: "flex", alignItems: "center", justifyContent: "center", padding: 0 }}>
        <Icon name="x" size={14} />
      </button>
    </div>
  );
}

function AddRule({ kind, onAdd }: { kind: "replace" | "snippet"; onAdd: (from: string, to: string) => void }) {
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const ok = from.trim() && to.trim();
  const add = () => { if (ok) { onAdd(from.trim(), to); setFrom(""); setTo(""); } };
  return (
    <div className="row" style={{ gap: 12, padding: "12px 16px" }}>
      <input className="input" style={{ flex: "1 1 0" }} placeholder={kind === "snippet" ? "Что говоришь: «моя подпись»" : "Что говоришь: «кубер»"}
        aria-label="Что говоришь" value={from} onChange={(e) => setFrom(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()} />
      <span className="faint" style={{ display: "flex" }}><Icon name="arrowr" size={16} /></span>
      {kind === "snippet" ? (
        <textarea className="input" style={{ flex: "1.4 1 0", height: 64, padding: "8px 12px", resize: "none" }} placeholder="Что вставить (можно в несколько строк)"
          aria-label="Что вставить" value={to} onChange={(e) => setTo(e.target.value)} />
      ) : (
        <input className="input" style={{ flex: "1.4 1 0" }} placeholder="Что получаешь: «Kubernetes»" aria-label="Что получаешь"
          value={to} onChange={(e) => setTo(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()} />
      )}
      <Button kind="subtle" size="sm" icon="plus" disabled={!ok} onClick={add}>Добавить</Button>
    </div>
  );
}

export function Dictionary() {
  const { settings, update } = useSettings();
  const [tab, setTab] = useState<Tab>("words");
  const [word, setWord] = useState("");
  const dict = settings.dictionary;

  const setDict = (patch: Partial<typeof dict>) => update((s) => ({ dictionary: { ...s.dictionary, ...patch } }));
  const addWord = () => {
    const w = word.trim();
    if (!w || dict.words.some((x) => x.toLowerCase() === w.toLowerCase())) return setWord("");
    setDict({ words: [...dict.words, w] });
    setWord("");
  };
  const rules = dict.rules.filter((r) => (tab === "snippet" ? r.kind === "snippet" : r.kind !== "snippet"));
  const setRule = (r: Rule) => setDict({ rules: dict.rules.map((x) => (x.id === r.id ? r : x)) });
  const delRule = (id: string) => setDict({ rules: dict.rules.filter((x) => x.id !== id) });
  const addRule = (kind: "replace" | "snippet") => (from: string, to: string) =>
    setDict({ rules: [...dict.rules, { id: crypto.randomUUID(), from, to, kind, enabled: true }] });

  return (
    <div className="page">
      <div className="page-head">
        <div className="col" style={{ gap: 6 }}>
          <h1>Словарь</h1>
          <span className="muted">Научи Маячок твоим словам, именам и сокращениям.</span>
        </div>
        <Segmented value={tab} onChange={setTab} options={[{ value: "words", label: "Слова" }, { value: "replace", label: "Замены" }, { value: "snippet", label: "Сниппеты" }]} />
      </div>

      {tab === "words" && (
        <div className="card col fade-in" style={{ padding: 20, gap: 16 }}>
          <div className="row" style={{ justifyContent: "space-between" }}>
            <h2 style={{ fontSize: 16 }}>Мои слова</h2>
            <span className="cap">Подсказки для распознавания · {dict.words.length}</span>
          </div>
          <p className="cap" style={{ margin: 0, lineHeight: 1.5 }}>
            Имена, бренды, термины — модель будет чаще писать их правильно. Работает как подсказка, а не жёсткая замена.
          </p>
          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            {dict.words.map((w) => (
              <span key={w} className="row" style={{ gap: 8, height: 34, padding: "0 8px 0 14px", borderRadius: 17, background: "var(--surf2)", fontSize: 14 }}>
                {w}
                <button type="button" aria-label={`Удалить ${w}`} onClick={() => setDict({ words: dict.words.filter((x) => x !== w) })}
                  style={{ width: 22, height: 22, border: 0, borderRadius: 11, background: "transparent", color: "var(--txt3)", display: "flex", alignItems: "center", justifyContent: "center", padding: 0 }}>
                  <Icon name="x" size={12} />
                </button>
              </span>
            ))}
            <label className="row" style={{ gap: 6, height: 34, padding: "0 14px", borderRadius: 17, border: "1px dashed var(--key-border)", color: "var(--txt3)" }}>
              <Icon name="plus" size={14} />
              <input value={word} onChange={(e) => setWord(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addWord()} onBlur={addWord}
                placeholder="Добавить слово" aria-label="Добавить слово"
                style={{ width: 140, border: 0, background: "transparent", color: "var(--txt)", fontSize: 14, outline: "none" }} />
            </label>
          </div>
        </div>
      )}

      {tab !== "words" && (
        <div className="card fade-in" style={{ overflow: "hidden" }}>
          <div className="row" style={{ justifyContent: "space-between", padding: "16px 16px 8px" }}>
            <h2 style={{ fontSize: 16 }}>{tab === "snippet" ? "Сниппеты" : "Замены и команды"}</h2>
            {tab === "replace" && !settings.voiceCommands && <span className="cap">Голосовые команды выключены в настройках</span>}
          </div>
          <div className="row cap" style={{ gap: 16, padding: "8px 16px", borderBottom: "1px solid var(--border)" }}>
            <span style={{ flex: "1 1 0" }}>Говоришь</span><span style={{ width: 16 }} /><span style={{ flex: "1.4 1 0" }}>Получаешь</span>
            <span style={{ width: 80 }}>Тип</span><span style={{ width: 72 }} />
          </div>
          {rules.map((r) => <RuleRow key={r.id} r={r} onChange={setRule} onDelete={() => delRule(r.id)} />)}
          {rules.length === 0 && <div className="cap" style={{ padding: 16 }}>Пока ничего нет</div>}
          <AddRule kind={tab === "snippet" ? "snippet" : "replace"} onAdd={addRule(tab === "snippet" ? "snippet" : "replace")} />
        </div>
      )}
    </div>
  );
}
