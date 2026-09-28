import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useLook } from "../App";
import { IslandView } from "../components/IslandView";
import { api, on, type IslandPayload, type IslandReminders } from "../lib/api";
import { useNow } from "../lib/remind";
import { useSettings } from "../lib/settings";
import { FireCard, PinDots, PinStack, SetCard } from "./RemIsland";

const HISTORY = 40;

/** Отдельное прозрачное окно поверх всех: диктовка и маячки */
export function IslandApp() {
  const { settings } = useSettings();
  const live = useLook(settings, false);
  const [p, setP] = useState<IslandPayload>({ phase: "hidden", mode: null, text: "", elapsedMs: 0, words: 0, message: "", app: "" });
  const [rem, setRem] = useState<IslandReminders>({ pins: [], firing: null, justSet: null });
  const [hover, setHover] = useState(false);
  const [levels, setLevels] = useState<number[]>([]);
  const buf = useRef<number[]>([]);
  const box = useRef<HTMLDivElement>(null);
  const now = useNow(1000);

  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    api.islandState().then(setP).catch(() => {});
    api.islandReminders().then(setRem).catch(() => {});
    const un1 = on<IslandPayload>("island", (x) => {
      setP((prev) => {
        if (x.phase === "listening" && prev.phase !== "listening") {
          buf.current = [];
          setLevels([]);
        }
        return x;
      });
    });
    const un2 = on<number>("level", (l) => {
      buf.current = [...buf.current.slice(-(HISTORY - 1)), l];
      setLevels(buf.current);
    });
    const un3 = on<IslandReminders>("island-reminders", setRem);
    // Бэкенд включает мышь, когда курсор над островом
    const un4 = on<boolean>("island-hover", setHover);
    return () => { un1(); un2(); un3(); un4(); };
  }, []);

  const dictating = p.phase === "listening" || p.phase === "processing" || p.phase === "done" || p.phase === "error";
  const mode = rem.firing ? "fire" : dictating ? "dictation" : rem.justSet ? "set" : rem.pins.length ? "pins" : p.phase === "idle" ? "idle" : "none";
  const interactive = mode === "fire" || mode === "pins";

  // Сообщаем, где нарисован остров: только там окно ловит мышь
  useLayoutEffect(() => {
    const el = box.current;
    if (!el || !interactive) {
      api.islandHit(null).catch(() => {});
      return;
    }
    const send = () => {
      const r = el.getBoundingClientRect();
      api.islandHit([r.left - 4, r.top - 4, r.width + 8, r.height + 8]).catch(() => {});
    };
    send();
    const ro = new ResizeObserver(send);
    ro.observe(el);
    // Морфинг размера — досылаем после анимации
    const t = window.setTimeout(send, 480);
    return () => { ro.disconnect(); window.clearTimeout(t); };
  }, [interactive, mode, hover, rem.pins.length]);

  const bottom = settings.island.position === "bottom";
  return (
    <div className="app" data-live={live} style={{
      height: "100vh", display: "flex", justifyContent: "center", alignItems: bottom ? "flex-end" : "flex-start",
      padding: bottom ? "0 0 8px" : "8px 0 0", boxSizing: "border-box",
    }}>
      <div ref={box} className="row" style={{ gap: 6, alignItems: "flex-start" }} onMouseLeave={() => setHover(false)}>
        {mode === "fire" && rem.firing && <FireCard r={rem.firing} settings={settings} />}
        {mode === "dictation" && (
          <>
            <IslandView p={p} levels={levels} settings={settings} />
            <PinDots pins={rem.pins} now={now} />
          </>
        )}
        {mode === "set" && rem.justSet && <SetCard r={rem.justSet} now={now} settings={settings} />}
        {mode === "pins" && <PinStack pins={rem.pins} now={now} expanded={hover} />}
        {mode === "idle" && <IslandView p={p} levels={levels} settings={settings} />}
      </div>
    </div>
  );
}
