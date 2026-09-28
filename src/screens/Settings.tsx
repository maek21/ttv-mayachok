import { ask, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState, type ReactNode } from "react";
import { IslandView } from "../components/IslandView";
import { Button, Group, Icon, Keys, Logo, Meter, Segmented, Select, SRow, Toggle } from "../components/ui";
import { api, type AppInfo, type HotkeyAction, type Settings as S, type UpdateInfo } from "../lib/api";
import { useHotkeyCapture, useMicLevel, useMics, useToast } from "../lib/hooks";
import { LANGUAGES } from "../lib/langs";
import { useSettings } from "../lib/settings";
import { CloudFields, ModelPicker } from "./Onboarding";

export type Section = "general" | "audio" | "hotkeys" | "island" | "privacy" | "about";

const SECTIONS: { id: Section; label: string }[] = [
  { id: "general", label: "Общие" },
  { id: "audio", label: "Звук и распознавание" },
  { id: "hotkeys", label: "Горячие клавиши" },
  { id: "island", label: "Остров" },
  { id: "privacy", label: "Приватность" },
  { id: "about", label: "О программе" },
];

function General() {
  const { settings: s, update } = useSettings();
  return (
    <>
      <Group>
        <SRow label="Запускать вместе с Windows" desc="Маячок стартует свёрнутым в трей."><Toggle checked={s.autostart} onChange={(v) => update({ autostart: v })} /></SRow>
        <SRow label="Закрытие сворачивает в трей" desc="Крестик не выключает диктовку."><Toggle checked={s.closeToTray} onChange={(v) => update({ closeToTray: v })} /></SRow>
        <SRow label="Язык интерфейса" desc="Другие языки появятся позже.">
          <Select width={200} value={s.uiLanguage} onChange={(v) => update({ uiLanguage: v })} options={[{ value: "ru", label: "Русский" }]} />
        </SRow>
        <SRow label="Тема">
          <Segmented value={s.theme} onChange={(v) => update({ theme: v })}
            options={[{ value: "dark", label: "Тёмная" }, { value: "light", label: "Светлая" }, { value: "system", label: "Системная" }]} />
        </SRow>
      </Group>
      <Group>
        <SRow label="Живая типографика" desc="Шрифт плавно «дышит» — ширина и грейд гуляют по кругу."><Toggle checked={s.liveType} onChange={(v) => update({ liveType: v })} /></SRow>
        <SRow label="Амплитуда">
          <div className="row" style={{ width: 240 }}>
            <input type="range" className="range" min={0.1} max={1} step={0.05} value={s.liveAmplitude} disabled={!s.liveType}
              aria-label="Амплитуда" onChange={(e) => update({ liveAmplitude: Number(e.target.value) })} />
            <span className="cap num" style={{ width: 36 }}>{Math.round(s.liveAmplitude * 100)}%</span>
          </div>
        </SRow>
        <SRow label="Замирать, когда окно не в фокусе" desc="Экономит батарею и не отвлекает."><Toggle checked={s.livePauseUnfocused} onChange={(v) => update({ livePauseUnfocused: v })} /></SRow>
        <SRow label="Звуки старта и стопа"><Toggle checked={s.sounds} onChange={(v) => update({ sounds: v })} /></SRow>
      </Group>
    </>
  );
}

function useAppInfo(deps: unknown[] = []) {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    let alive = true;
    const load = () => api.appInfo().then((i) => alive && setInfo(i)).catch(() => {});
    load();
    // Модель грузится в фоне — через пару секунд устройство уже известно
    const t = window.setTimeout(load, 2500);
    return () => { alive = false; window.clearTimeout(t); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return info;
}

function gpuDesc(info: AppInfo | null) {
  if (!info) return "…";
  if (!info.gpuBuild) return "Эта сборка только для процессора. Для видеокарты поставь версию с Vulkan.";
  if (info.gpuDevices.length === 0) return "Видеокарта с Vulkan не найдена — обнови драйвер. Пока считаю на процессоре.";
  return info.gpuDevices.join(", ");
}

function Audio() {
  const { settings: s, update } = useSettings();
  const mics = useMics();
  const { level, error } = useMicLevel(s.micDevice, true);
  const info = useAppInfo([s.useGpu, s.localModel, s.engine]);
  return (
    <>
      <Group>
        <SRow label="Микрофон">
          <Select width={300} label="Микрофон" value={s.micDevice ?? ""} onChange={(v) => update({ micDevice: v || null })}
            options={[{ value: "", label: "Системный по умолчанию" }, ...mics.map((m) => ({ value: m, label: m }))]} />
        </SRow>
        <SRow label="Уровень" desc={error ?? undefined}><Meter level={level} width={300} /></SRow>
        <SRow label="Фильтр шума" desc="Срезает низкочастотный гул и выравнивает громкость."><Toggle checked={s.noiseFilter} onChange={(v) => update({ noiseFilter: v })} /></SRow>
      </Group>
      <Group>
        <SRow label="Распознавание">
          <Segmented value={s.engine} onChange={(v) => update({ engine: v })} options={[{ value: "local", label: "Локально" }, { value: "cloud", label: "Облако" }]} />
        </SRow>
        <div className="srow" style={{ alignItems: "flex-start" }}>
          <div className="grow col" style={{ gap: 3 }}>
            <span className="label">{s.engine === "local" ? "Модель" : "Провайдер"}</span>
            <span className="desc">{s.engine === "local" ? "whisper.cpp, работает без интернета." : "Аудио уходит на указанный адрес."}</span>
          </div>
          <div style={{ width: 360 }}>{s.engine === "local" ? <ModelPicker compact /> : <CloudFields />}</div>
        </div>
        {s.engine === "local" && (
          <SRow label="Считать на видеокарте" desc={gpuDesc(info)}>
            <Toggle checked={s.useGpu && !!info?.gpuBuild} onChange={(v) => update({ useGpu: v })} label="Считать на видеокарте" />
          </SRow>
        )}
        {s.engine === "local" && info && (
          <SRow label="Сейчас считает" desc={info.backend.gpu ? undefined : `Потоков: ${info.backend.threads} · ${info.backend.cpuFeatures || "—"}`}>
            <span className="row" style={{ gap: 8, fontSize: 13, color: info.backend.gpu ? "var(--ok)" : "var(--txt2)" }}>
              <Icon name="cpu" size={16} />{info.loadedModel ? info.backend.device : "Модель ещё не загружена"}
            </span>
          </SRow>
        )}
        <SRow label="Язык речи" desc={s.autoDetect ? "Автоопределение включено — язык выберется сам." : undefined}>
          <Select width={200} label="Язык речи" value={s.language} onChange={(v) => update({ language: v })} options={LANGUAGES} />
        </SRow>
        <SRow label="Автоопределение языка"><Toggle checked={s.autoDetect} onChange={(v) => update({ autoDetect: v })} /></SRow>
        <SRow label="Авто-пунктуация"><Toggle checked={s.autoPunctuation} onChange={(v) => update({ autoPunctuation: v })} /></SRow>
        <SRow label="Убирать слова-паразиты" desc="«э-э», «ммм», «ну,», «типа,», «короче,»."><Toggle checked={s.removeFillers} onChange={(v) => update({ removeFillers: v })} /></SRow>
        <SRow label="Голосовые команды" desc="«новая строка», «новый абзац» и твои команды из словаря."><Toggle checked={s.voiceCommands} onChange={(v) => update({ voiceCommands: v })} /></SRow>
      </Group>
    </>
  );
}

const HOTKEYS: { id: HotkeyAction; label: string; desc?: string }[] = [
  { id: "dictate", label: "Диктовка", desc: "Удерживай и говори." },
  { id: "handsFree", label: "Режим без рук", desc: "Пишет, пока не нажмёшь снова." },
  { id: "cancel", label: "Отмена записи" },
  { id: "pasteLast", label: "Вставить последнее снова" },
  { id: "openApp", label: "Открыть Маячок" },
];

function Hotkeys() {
  const { settings: s, update } = useSettings();
  const cap = useHotkeyCapture();
  return (
    <>
      <Group>
        {HOTKEYS.map((h) => (
          <SRow key={h.id} label={h.label} desc={h.id === "dictate" && s.keyMode === "toggle" ? "Нажми — старт, нажми ещё раз — стоп." : h.desc}>
            {cap.action === h.id ? (
              <button type="button" onClick={cap.cancel} className="row" style={{ height: 38, boxSizing: "border-box", padding: "0 14px", borderRadius: 10, border: "2px solid var(--accent)", background: "transparent", gap: 8, fontSize: 13 }}>
                <span style={{ width: 7, height: 7, borderRadius: 4, background: "var(--accent)", animation: "pulse 1.2s infinite" }} />Нажми сочетание…
              </button>
            ) : (
              <div className="row">
                <Keys keys={s.hotkeys[h.id]} />
                <Button kind="ghost" size="sm" onClick={() => cap.start(h.id)}>Изменить</Button>
              </div>
            )}
          </SRow>
        ))}
      </Group>
      <Group>
        <SRow label="Режим клавиши диктовки">
          <Segmented value={s.keyMode} onChange={(v) => update({ keyMode: v })} options={[{ value: "hold", label: "Удерживать" }, { value: "toggle", label: "Нажать/нажать" }]} />
        </SRow>
        <SRow label="Буфер обмена после вставки" desc="Текст вставляется через буфер и Ctrl+V — так работает в любом приложении.">
          <Select width={240} value={s.clipboardMode} onChange={(v) => update({ clipboardMode: v })}
            options={[{ value: "restore", label: "Вернуть как было" }, { value: "keep", label: "Оставить продиктованное" }]} />
        </SRow>
        <SRow label="Сбросить сочетания">
          <Button kind="ghost" size="sm" onClick={() => update({ hotkeys: { dictate: ["Ctrl", "Win"], handsFree: ["Ctrl", "Win", "Space"], cancel: ["Esc"], pasteLast: ["Alt", "Shift", "V"], openApp: ["Ctrl", "Alt", "M"] } })}>По умолчанию</Button>
        </SRow>
      </Group>
    </>
  );
}

function PosCard({ label, pos, on, onClick }: { label: string; pos: S["island"]["position"]; on: boolean; onClick: () => void }) {
  const place = pos === "top" ? { top: 8, left: "50%", marginLeft: -22 } : pos === "bottom" ? { bottom: 8, left: "50%", marginLeft: -22 } : { top: 38, left: 70 };
  return (
    <button type="button" onClick={onClick} className="col" style={{ width: 132, gap: 8, border: 0, background: "transparent", padding: 0, color: "var(--txt2)", fontSize: 13 }}>
      <span style={{ position: "relative", display: "block", width: 132, height: 82, boxSizing: "border-box", borderRadius: 12, background: "var(--side)", border: on ? "2px solid var(--accent)" : "1px solid var(--border)" }}>
        <span style={{ position: "absolute", width: 44, height: 12, borderRadius: 6, background: "#000", border: "1px solid #3a3a40", ...place }} />
        {pos === "cursor" && <span style={{ position: "absolute", top: 24, left: 88, color: "var(--txt2)" }}><svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M4 2l16 10-7 1.5L9.5 21z" /></svg></span>}
      </span>
      {label}
    </button>
  );
}

const ACCENTS = ["#ff453a", "#0a84ff", "#30d158", "#ff9f0a", "#bf5af2"];

function Island() {
  const { settings: s, update } = useSettings();
  const set = (patch: Partial<S["island"]>) => update((x) => ({ island: { ...x.island, ...patch } }));
  return (
    <>
      <div className="card" style={{ height: 200, boxSizing: "border-box", background: "var(--side)", display: "flex", alignItems: s.island.position === "bottom" ? "flex-end" : "flex-start", justifyContent: "center", padding: 18, overflow: "hidden" }}>
        <IslandView preview settings={s} levels={[]}
          p={{ phase: "listening", mode: "hold", text: "Завтра в семь созвон, закинуть билд и проверить экспорт", elapsedMs: 7000, words: 0, message: "", app: "" }} />
      </div>
      <Group>
        <SRow label="Где показывать">
          <div className="row" style={{ gap: 12 }}>
            <PosCard label="Сверху" pos="top" on={s.island.position === "top"} onClick={() => set({ position: "top" })} />
            <PosCard label="Снизу" pos="bottom" on={s.island.position === "bottom"} onClick={() => set({ position: "bottom" })} />
            <PosCard label="У курсора" pos="cursor" on={s.island.position === "cursor"} onClick={() => set({ position: "cursor" })} />
          </div>
        </SRow>
        <SRow label="Размер" desc={s.island.size === "text" ? "Показывает распознанный текст прямо во время записи (только локально)." : undefined}>
          <Segmented value={s.island.size} onChange={(v) => set({ size: v })} options={[{ value: "compact", label: "Компакт" }, { value: "normal", label: "Обычный" }, { value: "text", label: "С текстом" }]} />
        </SRow>
        <SRow label="Цвет маячка">
          <div className="row" style={{ gap: 12 }}>
            {ACCENTS.map((c) => (
              <button key={c} type="button" aria-label={`Акцент ${c}`} onClick={() => set({ accent: c })} style={{
                width: 30, height: 30, borderRadius: 15, border: 0, padding: 0, background: c,
                boxShadow: s.island.accent === c ? `0 0 0 2px var(--surf), 0 0 0 4px ${c}` : "none",
              }} />
            ))}
          </div>
        </SRow>
        <SRow label="Прятать в ожидании" desc="Остров появляется только во время записи."><Toggle checked={s.island.hideIdle} onChange={(v) => set({ hideIdle: v })} /></SRow>
      </Group>
    </>
  );
}

function ListEditor({ items, onChange, placeholder }: { items: string[]; onChange: (v: string[]) => void; placeholder: string }) {
  const [v, setV] = useState("");
  const add = () => { const t = v.trim(); if (t && !items.includes(t)) onChange([...items, t]); setV(""); };
  return (
    <div className="row" style={{ gap: 6, flexWrap: "wrap", justifyContent: "flex-end", maxWidth: 380 }}>
      {items.map((i) => (
        <span key={i} className="row" style={{ gap: 4, height: 28, padding: "0 4px 0 10px", borderRadius: 14, background: "var(--surf2)", fontSize: 13 }}>
          {i}
          <button type="button" aria-label={`Убрать ${i}`} onClick={() => onChange(items.filter((x) => x !== i))} style={{ width: 20, height: 20, border: 0, borderRadius: 10, background: "transparent", color: "var(--txt3)", padding: 0, display: "flex", alignItems: "center", justifyContent: "center" }}><Icon name="x" size={11} /></button>
        </span>
      ))}
      <input className="input" style={{ height: 28, width: 130, fontSize: 13 }} placeholder={placeholder} aria-label={placeholder} value={v}
        onChange={(e) => setV(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()} onBlur={add} />
    </div>
  );
}

function hostOf(url: string) {
  try {
    return new URL(url).host;
  } catch {
    return url || "сервер провайдера";
  }
}

function Privacy() {
  const { settings: s, update } = useSettings();
  const toast = useToast();
  const exportHistory = async (format: "json" | "md") => {
    const path = await save({ defaultPath: `mayachok-history.${format}`, filters: [{ name: format === "md" ? "Markdown" : "JSON", extensions: [format] }] });
    if (!path) return;
    const n = await api.historyExport(path, format);
    toast.show(`Сохранено записей: ${n}`);
  };
  const clearAll = async () => {
    if (await ask("Удалить всю историю и аудио? Словарь останется.", { title: "Маячок", kind: "warning", okLabel: "Удалить", cancelLabel: "Отмена" })) {
      await api.historyClear();
      toast.show("История очищена");
    }
  };
  const setKeepAudio = async (v: boolean) => {
    if (!v && await ask("Удалить уже сохранённые аудиозаписи?", { title: "Маячок", okLabel: "Удалить", cancelLabel: "Оставить" })) {
      await api.historyDropAudio();
    }
    update({ keepAudio: v });
  };
  return (
    <>
      {s.engine === "local" ? (
        <div className="banner ok"><span style={{ color: "var(--ok)", display: "flex" }}><Icon name="shield" size={22} /></span>Распознавание локальное — голос и текст не покидают этот компьютер.</div>
      ) : (
        <div className="banner warn"><span style={{ color: "var(--warn)", display: "flex" }}><Icon name="cloud" size={22} /></span>Включено облачное распознавание — аудио отправляется на {hostOf(s.cloud.baseUrl)}.</div>
      )}
      <Group>
        <SRow label="Хранить историю">
          <Select width={200} value={String(s.historyDays)} onChange={(v) => update({ historyDays: Number(v) })}
            options={[{ value: "7", label: "7 дней" }, { value: "30", label: "30 дней" }, { value: "90", label: "90 дней" }, { value: "365", label: "Год" }, { value: "0", label: "Всегда" }]} />
        </SRow>
        <SRow label="Хранить аудиозаписи" desc="Нужно для прослушивания в истории. ~2 МБ в минуту."><Toggle checked={s.keepAudio} onChange={setKeepAudio} /></SRow>
        <SRow label="Не записывать в приложениях" desc="Менеджеры паролей, банки — остров не сработает. Имя как в истории или имя exe.">
          <ListEditor items={s.excludedApps} onChange={(v) => update({ excludedApps: v })} placeholder="Добавить" />
        </SRow>
      </Group>
      <Group>
        <SRow label="Экспорт истории" desc="JSON или Markdown.">
          <div className="row" style={{ gap: 8 }}>
            <Button kind="subtle" size="sm" icon="download" onClick={() => exportHistory("json")}>JSON</Button>
            <Button kind="subtle" size="sm" icon="download" onClick={() => exportHistory("md")}>Markdown</Button>
          </div>
        </SRow>
        <SRow label="Очистить всё" desc="Удалит историю и аудио. Словарь останется."><Button kind="danger" size="sm" icon="trash" onClick={clearAll}>Очистить</Button></SRow>
      </Group>
      {toast.msg && <div className="toast">{toast.msg}</div>}
    </>
  );
}

function About() {
  const { settings: s, update } = useSettings();
  const info = useAppInfo();
  const [upd, setUpd] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const check = () => {
    setChecking(true); setErr(null);
    api.checkUpdate().then(setUpd).catch((e) => setErr(String(e))).finally(() => setChecking(false));
  };
  return (
    <>
      <div className="card row" style={{ padding: 28, gap: 24 }}>
        <Logo size={84} />
        <div className="grow col" style={{ gap: 6 }}>
          <span className="head" style={{ fontSize: 34, letterSpacing: "-0.03em" }}>Маячок</span>
          <span className="muted">Версия {info?.version ?? "…"} · {info?.platform} · {info?.gpuBuild ? "сборка с Vulkan" : "сборка для процессора"}</span>
          {info && <span className="cap">{info.backend.device}{info.backend.gpu ? "" : ` · ${info.backend.cpuFeatures}`}</span>}
        </div>
        <Button kind="ghost" disabled={checking} onClick={check}>{checking ? "Проверяю…" : "Проверить обновления"}</Button>
      </div>
      {upd?.available && (
        <div className="card row fade-in" style={{ padding: "18px 20px", gap: 16, borderColor: "var(--accent)" }}>
          <span style={{ width: 40, height: 40, borderRadius: 12, background: "var(--surf2)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)" }}><Icon name="download" size={20} /></span>
          <div className="grow col" style={{ gap: 3 }}>
            <span style={{ fontSize: 15, fontWeight: 600 }}>Доступна версия {upd.latest}</span>
            {upd.notes && <span className="cap ellipsis" style={{ maxWidth: 520 }}>{upd.notes.split("\n")[0]}</span>}
          </div>
          <Button kind="primary" onClick={() => openUrl(upd.url)}>Скачать</Button>
        </div>
      )}
      {upd && !upd.available && <div className="banner ok fade-in"><Icon name="check" size={18} stroke={2.4} />Стоит последняя версия.</div>}
      {err && <div className="banner warn fade-in"><Icon name="alert" size={18} />Не удалось проверить: {err}</div>}
      <Group>
        <SRow label="Проверять обновления при запуске"><Toggle checked={s.autoUpdate} onChange={(v) => update({ autoUpdate: v })} /></SRow>
        <SRow label="Канал"><Segmented value={s.updateChannel} onChange={(v) => update({ updateChannel: v })} options={[{ value: "stable", label: "Стабильный" }, { value: "beta", label: "Бета" }]} /></SRow>
        <SRow label="Журнал работы" desc={info?.dataDir}><Button kind="ghost" size="sm" icon="folder" onClick={() => api.openLogs()}>Открыть папку логов</Button></SRow>
        <SRow label="Пройти приветствие заново"><Button kind="ghost" size="sm" onClick={() => update({ onboarded: false })}>Запустить</Button></SRow>
      </Group>
    </>
  );
}

const TITLES: Record<Section, string> = Object.fromEntries(SECTIONS.map((x) => [x.id, x.label])) as Record<Section, string>;
const BODIES: Record<Section, () => ReactNode> = {
  general: General, audio: Audio, hotkeys: Hotkeys, island: Island, privacy: Privacy, about: About,
};

export function Settings({ section, setSection }: { section: Section; setSection: (s: Section) => void }) {
  const Body = BODIES[section];
  return (
    <div className="grow" style={{ display: "flex", minHeight: 0 }}>
      <div className="col" style={{ width: 220, flexShrink: 0, boxSizing: "border-box", padding: "12px 16px", gap: 2, borderRight: "1px solid var(--border-soft)" }}>
        <h1 style={{ fontSize: 24, padding: "0 12px 14px" }}>Настройки</h1>
        {SECTIONS.map((x) => (
          <button key={x.id} type="button" className={`nav ${section === x.id ? "active" : ""}`} style={{ height: 36 }} onClick={() => setSection(x.id)}>{x.label}</button>
        ))}
      </div>
      <div className="grow col fade-in" key={section} style={{ boxSizing: "border-box", padding: "16px 48px 32px", gap: 20, overflow: "auto" }}>
        <h2 style={{ fontSize: 22 }}>{TITLES[section]}</h2>
        <Body />
      </div>
    </div>
  );
}
