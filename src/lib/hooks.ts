import { useCallback, useEffect, useRef, useState } from "react";
import { api, on, type HotkeyAction, type ModelInfo, type ModelProgress } from "./api";
import { useSettings } from "./settings";

/** Тест микрофона: держит устройство открытым, пока компонент на экране */
export function useMicLevel(device: string | null, active = true) {
  const [level, setLevel] = useState(0);
  const [peak, setPeak] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState<string | null>(null);

  useEffect(() => {
    if (!active) return;
    let alive = true;
    let timer: number | undefined;
    setError(null);
    api.micTestStart(device)
      .then((n) => {
        if (!alive) return;
        setName(n);
        timer = window.setInterval(async () => {
          const l = await api.micTestLevel().catch(() => 0);
          if (!alive) return;
          setLevel(l);
          setPeak((p) => Math.max(p * 0.97, l));
        }, 60);
      })
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
      window.clearInterval(timer);
      api.micTestStop().catch(() => {});
    };
  }, [device, active]);

  return { level, peak, error, name };
}

export function useMics() {
  const [mics, setMics] = useState<string[]>([]);
  useEffect(() => {
    api.listMics().then(setMics).catch(() => setMics([]));
  }, []);
  return mics;
}

export function useModels() {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [progress, setProgress] = useState<ModelProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    api.modelsList().then(setModels).catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
    api.modelDownloading().then((id) => {
      if (id) setProgress({ id, downloaded: 0, total: 0, done: false, error: null });
    });
    return on<ModelProgress>("model-progress", (p) => {
      if (p.error) {
        setError(p.error);
        setProgress(null);
      } else if (p.done) {
        setProgress(null);
        refresh();
      } else {
        setError(null);
        setProgress(p);
      }
    });
  }, [refresh]);

  const download = (id: string) => {
    setError(null);
    setProgress({ id, downloaded: 0, total: 0, done: false, error: null });
    api.modelDownload(id).catch((e) => {
      setError(String(e));
      setProgress(null);
    });
  };
  const cancel = () => api.modelCancel();
  const remove = (id: string) => api.modelDelete(id).then(refresh);

  return { models, progress, error, download, cancel, remove, refresh };
}

/** Запись нового сочетания клавиш через системный хук */
export function useHotkeyCapture() {
  const { update } = useSettings();
  const [action, setAction] = useState<HotkeyAction | null>(null);
  const actionRef = useRef<HotkeyAction | null>(null);

  useEffect(() => {
    const un = on<string[]>("hotkey-captured", (keys) => {
      const a = actionRef.current;
      actionRef.current = null;
      setAction(null);
      if (!a || keys.length === 0) return;
      // Один Esc для любого действия, кроме «Отмены», — значит передумал
      if (keys.length === 1 && keys[0] === "Esc" && a !== "cancel") return;
      update((s) => ({ hotkeys: { ...s.hotkeys, [a]: keys } }));
    });
    return () => {
      un();
      if (actionRef.current) api.hotkeyCaptureCancel();
    };
  }, [update]);

  const start = (a: HotkeyAction) => {
    actionRef.current = a;
    setAction(a);
    api.hotkeyCaptureStart();
  };
  const cancel = () => {
    actionRef.current = null;
    setAction(null);
    api.hotkeyCaptureCancel();
  };
  return { action, start, cancel };
}

export function useToast() {
  const [msg, setMsg] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const show = useCallback((m: string) => {
    setMsg(m);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setMsg(null), 2200);
  }, []);
  return { msg, show };
}
