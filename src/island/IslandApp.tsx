import { useEffect, useRef, useState } from "react";
import { useLook } from "../App";
import { IslandView } from "../components/IslandView";
import { api, on, type IslandPayload } from "../lib/api";
import { useSettings } from "../lib/settings";

const HISTORY = 40;

/** Отдельное прозрачное окно поверх всех: только остров */
export function IslandApp() {
  const { settings } = useSettings();
  const live = useLook(settings, false);
  const [p, setP] = useState<IslandPayload>({ phase: "hidden", mode: null, text: "", elapsedMs: 0, words: 0, message: "", app: "" });
  const [levels, setLevels] = useState<number[]>([]);
  const buf = useRef<number[]>([]);

  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    api.islandState().then(setP).catch(() => {});
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
    return () => { un1(); un2(); };
  }, []);

  const bottom = settings.island.position === "bottom";
  return (
    <div className="app" data-live={live} style={{
      height: "100vh", display: "flex", justifyContent: "center", alignItems: bottom ? "flex-end" : "flex-start",
      padding: bottom ? "0 0 8px" : "8px 0 0", boxSizing: "border-box",
    }}>
      {p.phase !== "hidden" && <IslandView p={p} levels={levels} settings={settings} />}
    </div>
  );
}
