import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { IslandPayload, Settings } from "../lib/api";
import { fmtDuration, plural } from "../lib/settings";
import { Icon, Keys } from "./ui";

export function Bars({ levels, count, width = 3, height = 28, gap = 3, color = "#f5f5f7" }: {
  levels: number[]; count: number; width?: number; height?: number; gap?: number; color?: string;
}) {
  const data = levels.slice(-count);
  while (data.length < count) data.unshift(0);
  return (
    <div style={{ display: "flex", alignItems: "center", gap, height, flexGrow: 1, overflow: "hidden" }}>
      {data.map((l, i) => {
        const v = Math.max(0.12, Math.min(1, Math.sqrt(l) * 2.4));
        return (
          <div key={i} style={{
            width, flexShrink: 0, borderRadius: width, background: color,
            height: Math.round(4 + v * (height - 4)), transition: "height 90ms linear",
          }} />
        );
      })}
    </div>
  );
}

/** Декоративная волна для превью, без реального звука */
export function FakeBars({ count, width = 3, height = 28, gap = 3, seed = 0.4 }: { count: number; width?: number; height?: number; gap?: number; seed?: number }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap, height, flexGrow: 1, overflow: "hidden" }}>
      {Array.from({ length: count }, (_, i) => (
        <div key={i} style={{
          width, flexShrink: 0, borderRadius: width, background: "#f5f5f7",
          height: 4 + Math.round(Math.abs(Math.sin(i * 1.7 + seed)) * (height - 6)),
          animation: `wv 0.9s ease-in-out ${((i % 7) * 0.11).toFixed(2)}s infinite`,
        }} />
      ))}
    </div>
  );
}

const Dot = () => (
  <div style={{ width: 8, height: 8, borderRadius: 4, background: "var(--accent)", animation: "pulse 1.2s ease-in-out infinite", flexShrink: 0 }} />
);

function Pill({ children }: { children: ReactNode }) {
  return <div className="island-row" style={{ display: "flex", alignItems: "center", gap: 10, height: 40, padding: "0 16px", whiteSpace: "nowrap", fontSize: 13 }}>{children}</div>;
}

interface Props {
  p: IslandPayload;
  levels: number[];
  settings: Settings;
  /** true — превью внутри окна приложения: волна крутится сама */
  preview?: boolean;
}

/** Остров, плавно меняющий размер под содержимое — как Dynamic Island */
export function IslandView({ p, levels, settings, preview }: Props) {
  const inner = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 120, h: 36 });

  useLayoutEffect(() => {
    const el = inner.current;
    if (!el) return;
    const measure = () => setBox({ w: el.offsetWidth, h: el.offsetHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [p.phase, settings.island.size]);

  const size = settings.island.size;
  const locked = p.mode === "locked";
  const dictate = settings.hotkeys.dictate;
  let content: ReactNode;

  if (p.phase === "listening" && size === "text") {
    content = (
      <div style={{ width: 560, boxSizing: "border-box", padding: "16px 20px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          {locked ? <span style={{ color: "var(--accent)", display: "flex" }}><Icon name="lock" size={15} /></span> : <Dot />}
          <span style={{ fontSize: 13, color: "#8e8e93" }}>{locked ? "Без рук" : "Слушаю"}</span>
          {preview ? <FakeBars count={28} /> : <Bars levels={levels} count={28} />}
          <span className="num" style={{ fontSize: 13, color: "#8e8e93" }}>{fmtDuration(p.elapsedMs)}</span>
        </div>
        <div style={{ fontSize: 19, lineHeight: 1.4, fontWeight: 500, minHeight: 27, maxHeight: 108, overflow: "hidden", display: "flex", flexDirection: "column", justifyContent: "flex-end" }}>
          <span>
            {p.text || <span style={{ color: "#8e8e93" }}>Говори…</span>}
            <span style={{ display: "inline-block", width: 2, height: 20, marginLeft: 3, verticalAlign: -3, background: "var(--accent)", animation: "blink 1s steps(1) infinite" }} />
          </span>
        </div>
        <div style={{ display: "flex", gap: 16, fontSize: 12, color: "#8e8e93" }}>
          {locked || p.mode === "toggle"
            ? <span className="row" style={{ gap: 6 }}><Keys keys={dictate} />стоп и вставить</span>
            : <span className="row" style={{ gap: 6 }}><span className="key">Отпусти</span>вставить</span>}
          <span className="row" style={{ gap: 6 }}><Keys keys={settings.hotkeys.cancel} />отмена</span>
          {!locked && <span className="row" style={{ gap: 6 }}><Keys keys={settings.hotkeys.handsFree} />без рук</span>}
        </div>
      </div>
    );
  } else if (p.phase === "listening") {
    const wide = size === "normal";
    content = (
      <Pill>
        {locked ? <span style={{ color: "var(--accent)", display: "flex" }}><Icon name="lock" size={14} /></span> : <Dot />}
        <div style={{ width: wide ? 150 : 90, display: "flex" }}>
          {preview ? <FakeBars count={wide ? 30 : 18} width={2} height={18} gap={2} /> : <Bars levels={levels} count={wide ? 30 : 18} width={2} height={18} gap={2} />}
        </div>
        <span className="num" style={{ fontSize: 11, color: "#8e8e93" }}>{fmtDuration(p.elapsedMs)}</span>
      </Pill>
    );
  } else if (p.phase === "processing") {
    content = (
      <Pill>
        <span style={{ width: 14, height: 14, boxSizing: "border-box", borderRadius: 7, border: "2px solid #3a3a3c", borderTopColor: "#f5f5f7", animation: "spin .8s linear infinite" }} />
        {size === "compact" ? "Секунду…" : p.message || "Распознаю…"}
        {size !== "compact" && <span className="row" style={{ gap: 6, color: "#8e8e93", fontSize: 12, marginLeft: 4 }}><Keys keys={settings.hotkeys.cancel} />отмена</span>}
      </Pill>
    );
  } else if (p.phase === "done") {
    content = (
      <Pill>
        <span style={{ color: "#30d158", display: "flex" }}><Icon name="check" size={16} stroke={2.4} /></span>
        {p.message}{p.words > 0 ? ` · ${p.words} ${plural(p.words, "слово", "слова", "слов")}` : ""}
      </Pill>
    );
  } else if (p.phase === "error") {
    content = (
      <Pill>
        <span style={{ color: "#ff9f0a", display: "flex" }}><Icon name="alert" size={16} /></span>
        <span style={{ maxWidth: 420, overflow: "hidden", textOverflow: "ellipsis" }}>{p.message}</span>
      </Pill>
    );
  } else {
    content = (
      <div style={{ width: 120, height: 36, display: "flex", alignItems: "center", justifyContent: "center", color: "#8e8e93" }}>
        <Icon name="mic" size={16} />
      </div>
    );
  }

  const radius = Math.min(30, box.h / 2);
  return (
    <div style={{
      width: box.w, height: box.h, borderRadius: radius, background: "#000", color: "#f5f5f7", overflow: "hidden",
      boxShadow: "0 18px 50px rgba(0,0,0,.5)", position: "relative",
      transition: "width .42s cubic-bezier(.2,.9,.25,1.12), height .42s cubic-bezier(.2,.9,.25,1.12), border-radius .42s",
      fontFamily: "var(--font-body)",
      // Остров всегда чёрный — клавиши внутри тоже тёмные, в любой теме
      ["--key-bg" as string]: "#1c1c1e", ["--key-border" as string]: "#3a3a3c", ["--txt2" as string]: "#d1d1d6", ["--txt3" as string]: "#8e8e93",
    }}>
      <div ref={inner} key={`${p.phase}-${size}`} className="fade-in" style={{ position: "absolute", left: 0, top: 0, width: "max-content" }}>
        {content}
      </div>
    </div>
  );
}
