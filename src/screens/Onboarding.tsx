import { useEffect, useState, type ReactNode } from "react";
import { TitleBar, useEngineState } from "../components/Chrome";
import { IslandView } from "../components/IslandView";
import { Button, Icon, Keys, Meter, Progress, Select, Toggle, type IconName } from "../components/ui";
import { useHotkeyCapture, useMicLevel, useMics, useModels } from "../lib/hooks";
import { LANGUAGES } from "../lib/langs";
import { useSettings } from "../lib/settings";

const STEPS = 5;

function Dots({ step }: { step: number }) {
  return (
    <div className="row" style={{ gap: 6 }}>
      {Array.from({ length: STEPS - 1 }, (_, i) => (
        <span key={i} style={{
          width: i === step - 1 ? 22 : 7, height: 7, borderRadius: 4, transition: "width .3s",
          background: i === step - 1 ? "var(--accent)" : "var(--key-border)",
        }} />
      ))}
    </div>
  );
}

function Frame({ step, setStep, children, next, nextLabel = "Далее", canNext = true, finish }: {
  step: number; setStep: (n: number) => void; children: ReactNode; next?: () => void;
  nextLabel?: string; canNext?: boolean; finish: () => void;
}) {
  return (
    <>
      <div className="grow fade-in" key={step} style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 28, padding: "0 80px", minHeight: 0 }}>
        {children}
      </div>
      <div style={{ height: 88, flexShrink: 0, boxSizing: "border-box", padding: "0 40px", display: "flex", alignItems: "center", borderTop: "1px solid var(--border-soft)" }}>
        <div style={{ width: 280 }}><Button kind="ghost" onClick={() => setStep(step - 1)}>Назад</Button></div>
        <div className="grow" style={{ display: "flex", justifyContent: "center" }}><Dots step={step} /></div>
        <div className="row" style={{ width: 280, justifyContent: "flex-end", gap: 10 }}>
          {step < STEPS - 1 && <Button kind="link" onClick={finish}>Пропустить</Button>}
          <Button kind="primary" disabled={!canNext} onClick={next ?? (() => setStep(step + 1))}>{nextLabel}</Button>
        </div>
      </div>
    </>
  );
}

function Heading({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="col" style={{ alignItems: "center", gap: 12 }}>
      <h1 style={{ fontSize: 40 }}>{title}</h1>
      {children && <p style={{ margin: 0, maxWidth: 560, textAlign: "center", fontSize: 16, lineHeight: 1.5, color: "var(--txt2)" }}>{children}</p>}
    </div>
  );
}

export function SweepWord({ word, size }: { word: string; size: number }) {
  return (
    <div className="head sweep" style={{ fontSize: size, letterSpacing: "-0.04em", lineHeight: 1 }} aria-label={word}>
      {[...word].map((c, i) => <span key={i} aria-hidden="true" style={{ animationDelay: `${(i * 0.18).toFixed(2)}s` }}>{c}</span>)}
    </div>
  );
}

function Welcome({ go }: { go: () => void }) {
  const { settings } = useSettings();
  return (
    <>
      <div className="grow fade-in" style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 36 }}>
        <div style={{ transform: "scale(.9)" }}>
          <IslandView preview settings={{ ...settings, island: { ...settings.island, size: "compact" } }} levels={[]}
            p={{ phase: "listening", mode: "hold", text: "", elapsedMs: 7000, words: 0, message: "", app: "" }} />
        </div>
        <div className="col" style={{ alignItems: "center", gap: 18 }}>
          <SweepWord word="Маячок" size={132} />
          <p style={{ margin: 0, maxWidth: 560, textAlign: "center", fontSize: 20, lineHeight: 1.45, color: "var(--txt2)" }}>
            Говори — текст появится там, где стоит курсор. В любом приложении Windows, без переключений и копипасты.
          </p>
        </div>
        <div className="col" style={{ alignItems: "center", gap: 14 }}>
          <Button kind="accent" size="lg" icon="arrowr" onClick={go}>Начнём</Button>
          <span className="cap">Настройка займёт минуту</span>
        </div>
      </div>
      <div className="row cap" style={{ height: 56, justifyContent: "center", gap: 18 }}>
        <span>Windows 10/11</span><span>·</span><span>Распознавание можно держать полностью локально</span>
      </div>
    </>
  );
}

function StepMic(props: FrameProps) {
  const { settings, update } = useSettings();
  const mics = useMics();
  const { level, peak, error } = useMicLevel(settings.micDevice, true);
  const good = peak > 0.02;
  return (
    <Frame {...props}>
      <div style={{ width: 96, height: 96, borderRadius: 48, background: "var(--surf2)", display: "flex", alignItems: "center", justifyContent: "center" }}>
        <Icon name="mic" size={40} stroke={1.6} />
      </div>
      <Heading title="Дай Маячку послушать">
        Выбери микрофон и скажи пару слов — полоска должна прыгать. Звук пишется только пока зажата клавиша.
      </Heading>
      <div className="card col" style={{ width: 460, boxSizing: "border-box", padding: 20, gap: 16, alignItems: "center" }}>
        <Select width={420} label="Микрофон" value={settings.micDevice ?? ""} onChange={(v) => update({ micDevice: v || null })}
          options={[{ value: "", label: "Системный по умолчанию" }, ...mics.map((m) => ({ value: m, label: m }))]} />
        <Meter level={level} width={420} />
        {error ? (
          <span className="row cap" style={{ color: "var(--warn)", gap: 6 }}><Icon name="alert" size={14} />{error}</span>
        ) : good ? (
          <span className="row cap" style={{ color: "var(--ok)", gap: 6 }}><Icon name="check" size={14} stroke={2.4} />Слышу тебя хорошо</span>
        ) : (
          <span className="cap">Скажи что-нибудь…</span>
        )}
      </div>
    </Frame>
  );
}

function ModeCard({ title, desc, on, onClick }: { title: string; desc: ReactNode; on: boolean; onClick?: () => void }) {
  return (
    <button type="button" onClick={onClick} disabled={!onClick} style={{
      width: 300, boxSizing: "border-box", padding: 18, borderRadius: 16, background: "var(--surf)", color: "var(--txt)",
      border: on ? "2px solid var(--accent)" : "1px solid var(--border)", textAlign: "left", display: "flex", gap: 14, alignItems: "flex-start",
      opacity: onClick || on ? 1 : 0.85,
    }}>
      <span style={{ width: 18, height: 18, boxSizing: "border-box", borderRadius: 9, flexShrink: 0, border: on ? "5px solid var(--accent)" : "2px solid var(--key-border)" }} />
      <span className="col" style={{ gap: 6 }}>
        <span style={{ fontSize: 15, fontWeight: 600 }}>{title}</span>
        <span style={{ fontSize: 13, lineHeight: 1.45, color: "var(--txt2)" }}>{desc}</span>
      </span>
    </button>
  );
}

function StepHotkey(props: FrameProps) {
  const { settings, update } = useSettings();
  const cap = useHotkeyCapture();
  return (
    <Frame {...props}>
      <Heading title="Одна клавиша на всё">Зажал — говоришь, отпустил — текст вставлен. Сочетание можно поменять.</Heading>
      {cap.action === "dictate" ? (
        <div className="row" style={{ height: 94, gap: 12, fontSize: 18, color: "var(--txt2)" }}>
          <span style={{ width: 10, height: 10, borderRadius: 5, background: "var(--accent)", animation: "pulse 1.2s infinite" }} />
          Нажми и отпусти новое сочетание…
        </div>
      ) : (
        <div style={{ paddingBottom: 6 }}><Keys keys={settings.hotkeys.dictate} big /></div>
      )}
      {cap.action === "dictate"
        ? <Button kind="subtle" size="sm" onClick={cap.cancel}>Отмена</Button>
        : <Button kind="subtle" size="sm" icon="keyboard" onClick={() => cap.start("dictate")}>Задать своё сочетание</Button>}
      <div className="row" style={{ gap: 16, alignItems: "stretch" }}>
        <ModeCard title="Удерживать" desc="Говоришь, пока держишь. Идеально для коротких фраз." on={settings.keyMode === "hold"} onClick={() => update({ keyMode: "hold" })} />
        <ModeCard title="Нажать и нажать" desc="Первое нажатие — старт, второе — стоп." on={settings.keyMode === "toggle"} onClick={() => update({ keyMode: "toggle" })} />
        <ModeCard title="Без рук" desc={<>{settings.hotkeys.handsFree.join(" + ")} — пишет, пока не нажмёшь снова. Работает в любом режиме.</>} on={false} />
      </div>
    </Frame>
  );
}

function ChoiceCard({ icon, title, meta, desc, on, onClick, children, badge }: {
  icon: IconName; title: string; meta: string; desc: string; on: boolean; onClick: () => void; children?: ReactNode; badge?: string;
}) {
  return (
    <div role="radio" aria-checked={on} tabIndex={0} onClick={onClick} onKeyDown={(e) => e.key === "Enter" && onClick()} style={{
      width: 400, boxSizing: "border-box", padding: 22, borderRadius: 18, background: "var(--surf)",
      border: on ? "2px solid var(--accent)" : "1px solid var(--border)", display: "flex", flexDirection: "column", gap: 12,
    }}>
      <div className="row">
        <span style={{ width: 40, height: 40, borderRadius: 12, background: "var(--surf2)", display: "flex", alignItems: "center", justifyContent: "center" }}><Icon name={icon} size={20} /></span>
        <div className="grow col" style={{ gap: 2 }}><h2 style={{ fontSize: 17 }}>{title}</h2><span className="cap">{meta}</span></div>
        {badge && <span style={{ padding: "4px 8px", borderRadius: 6, background: "color-mix(in srgb, var(--accent) 18%, transparent)", color: "var(--accent)", fontSize: 11, fontWeight: 600 }}>{badge}</span>}
      </div>
      <p style={{ margin: 0, fontSize: 13, lineHeight: 1.5, color: "var(--txt2)" }}>{desc}</p>
      {on && children}
    </div>
  );
}

export function ModelPicker({ compact }: { compact?: boolean }) {
  const { settings, update } = useSettings();
  const { models, progress, error, download, cancel } = useModels();
  const current = models.find((m) => m.id === settings.localModel);
  const busy = progress?.id === settings.localModel;
  return (
    <div className="col" style={{ gap: 10 }} onClick={(e) => e.stopPropagation()}>
      <Select width={compact ? 280 : 354} label="Модель" value={settings.localModel} onChange={(v) => update({ localModel: v })}
        options={models.map((m) => ({ value: m.id, label: `${m.name} · ${m.sizeMb >= 1000 ? (m.sizeMb / 1024).toFixed(1).replace(".", ",") + " ГБ" : m.sizeMb + " МБ"}${m.installed ? " ✓" : ""}` }))} />
      {current && !compact && <span className="cap">{current.desc}</span>}
      {busy && progress ? (
        <div className="col" style={{ gap: 8 }}>
          <Progress value={progress.total ? progress.downloaded / progress.total : 0} />
          <div className="row cap num" style={{ justifyContent: "space-between" }}>
            <span>Скачиваю · {(progress.downloaded / 1048576).toFixed(0)} из {(progress.total / 1048576 || current?.sizeMb || 0).toFixed(0)} МБ</span>
            <button type="button" className="btn link" style={{ height: 20, padding: 0 }} onClick={cancel}>Отменить</button>
          </div>
        </div>
      ) : current?.installed ? (
        <span className="row cap" style={{ color: "var(--ok)", gap: 6 }}><Icon name="check" size={14} stroke={2.4} />Модель скачана, работает офлайн</span>
      ) : (
        <div className="row"><Button kind="subtle" size="sm" icon="download" disabled={!!progress} onClick={() => download(settings.localModel)}>Скачать модель</Button>
          {progress && <span className="cap">Сначала дождись другой загрузки</span>}</div>
      )}
      {error && <span className="row cap" style={{ color: "var(--warn)", gap: 6 }}><Icon name="alert" size={14} />{error}</span>}
    </div>
  );
}

export function CloudFields() {
  const { settings, update } = useSettings();
  const set = (patch: Partial<typeof settings.cloud>) => update((s) => ({ cloud: { ...s.cloud, ...patch } }));
  return (
    <div className="col" style={{ gap: 8 }} onClick={(e) => e.stopPropagation()}>
      <input className="input" placeholder="API-ключ" type="password" aria-label="API-ключ" value={settings.cloud.apiKey} onChange={(e) => set({ apiKey: e.target.value })} />
      <div className="row" style={{ gap: 8 }}>
        <input className="input grow" aria-label="Адрес API" value={settings.cloud.baseUrl} onChange={(e) => set({ baseUrl: e.target.value })} />
        <input className="input" style={{ width: 130 }} aria-label="Модель" value={settings.cloud.model} onChange={(e) => set({ model: e.target.value })} />
      </div>
      <span className="cap">Любой OpenAI-совместимый /audio/transcriptions: OpenAI, Groq и т.п.</span>
    </div>
  );
}

function StepModel(props: FrameProps) {
  const { settings, update } = useSettings();
  const { models } = useModels();
  const ready = settings.engine === "cloud" ? settings.cloud.apiKey.trim().length > 0 : !!models.find((m) => m.id === settings.localModel)?.installed;
  return (
    <Frame {...props} canNext={ready}>
      <Heading title="Где распознавать речь?">Потом можно переключить в настройках.</Heading>
      <div className="row" style={{ gap: 20, alignItems: "flex-start" }}>
        <ChoiceCard icon="cpu" title="Локально" meta="whisper.cpp · офлайн" badge="Рекомендуем" on={settings.engine === "local"} onClick={() => update({ engine: "local" })}
          desc="Голос не покидает компьютер. Модель скачивается один раз.">
          <ModelPicker />
        </ChoiceCard>
        <ChoiceCard icon="cloud" title="Облако" meta="Нужен интернет и API-ключ" on={settings.engine === "cloud"} onClick={() => update({ engine: "cloud" })}
          desc="Быстрее на слабых ноутбуках. Аудио уходит провайдеру распознавания.">
          <CloudFields />
        </ChoiceCard>
      </div>
      <div className="row" style={{ gap: 28 }}>
        <div className="row"><span className="muted">Язык</span>
          <Select width={200} label="Язык" value={settings.language} onChange={(v) => update({ language: v })} options={LANGUAGES} /></div>
        <div className="row"><Toggle checked={settings.autoDetect} onChange={(v) => update({ autoDetect: v })} label="Автоопределение языка" /><span className="muted">Автоопределение</span></div>
        <div className="row"><Toggle checked={settings.autoPunctuation} onChange={(v) => update({ autoPunctuation: v })} label="Авто-пунктуация" /><span className="muted">Авто-пунктуация</span></div>
      </div>
    </Frame>
  );
}

function StepTry(props: FrameProps) {
  const { settings, update } = useSettings();
  const engine = useEngineState();
  const [text, setText] = useState("");
  const live = engine && engine.phase !== "hidden" && engine.phase !== "idle";
  return (
    <Frame {...props}>
      <Heading title="Попробуй прямо здесь" />
      <p style={{ margin: "-16px 0 0", textAlign: "center", fontSize: 16, color: "var(--txt2)" }}>
        Кликни в поле, {settings.keyMode === "hold" ? "зажми" : "нажми"} <Keys keys={settings.hotkeys.dictate} /> и скажи: «Привет, Маячок, это проверка».
      </p>
      <div className="col" style={{ width: 640, alignItems: "center", gap: 0 }}>
        <div style={{ height: 60, display: "flex", alignItems: "flex-end", marginBottom: -18, zIndex: 1 }}>
          {live && engine && <IslandView settings={{ ...settings, island: { ...settings.island, size: "normal" } }} levels={[]} preview p={engine} />}
        </div>
        <textarea autoFocus value={text} onChange={(e) => setText(e.target.value)} placeholder="Здесь появится текст…" aria-label="Проверка диктовки" style={{
          width: 640, boxSizing: "border-box", padding: "30px 20px 20px", minHeight: 150, borderRadius: 16, resize: "none",
          background: "var(--surf)", border: "1px solid var(--accent)", fontSize: 17, lineHeight: 1.5, outline: "none",
        }} />
      </div>
      {text.trim().length > 0 && <span className="row" style={{ color: "var(--ok)", gap: 6 }}><Icon name="check" size={16} stroke={2.4} />Работает! Так же будет в любом окне.</span>}
      <div className="row"><Toggle checked={settings.autostart} onChange={(v) => update({ autostart: v })} label="Автозапуск" /><span className="muted">Запускать Маячок вместе с Windows</span></div>
    </Frame>
  );
}

interface FrameProps { step: number; setStep: (n: number) => void; finish: () => void; nextLabel?: string; next?: () => void }

export function Onboarding() {
  const { update } = useSettings();
  const [step, setStep] = useState(0);
  const finish = () => update({ onboarded: true });

  useEffect(() => {
    if (step < 0) setStep(0);
  }, [step]);

  const props: FrameProps = { step, setStep, finish };
  return (
    <div className="window" style={{ flexDirection: "column" }}>
      <TitleBar title={step === 0 ? "" : "Маячок"} />
      {step === 0 && <Welcome go={() => setStep(1)} />}
      {step === 1 && <StepMic {...props} />}
      {step === 2 && <StepHotkey {...props} />}
      {step === 3 && <StepModel {...props} />}
      {step === 4 && <StepTry {...props} nextLabel="Готово, погнали" next={finish} />}
    </div>
  );
}
