import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type HotkeyAction = "dictate" | "handsFree" | "cancel" | "pasteLast" | "openApp";

export interface Rule {
  id: string;
  from: string;
  to: string;
  kind: "command" | "replace" | "snippet";
  enabled: boolean;
}

export interface Settings {
  onboarded: boolean;
  autostart: boolean;
  closeToTray: boolean;
  uiLanguage: string;
  theme: "dark" | "light" | "system";
  liveType: boolean;
  liveAmplitude: number;
  livePauseUnfocused: boolean;
  sounds: boolean;
  micDevice: string | null;
  noiseFilter: boolean;
  engine: "local" | "cloud";
  localModel: string;
  useGpu: boolean;
  cloud: { baseUrl: string; apiKey: string; model: string };
  language: string;
  autoDetect: boolean;
  autoPunctuation: boolean;
  removeFillers: boolean;
  voiceCommands: boolean;
  hotkeys: Record<HotkeyAction, string[]>;
  keyMode: "hold" | "toggle";
  clipboardMode: "restore" | "keep";
  island: { position: "top" | "bottom" | "cursor"; size: "compact" | "normal" | "text"; accent: string; hideIdle: boolean };
  historyDays: number;
  keepAudio: boolean;
  excludedApps: string[];
  dictionary: { words: string[]; rules: Rule[] };
  autoUpdate: boolean;
  updateChannel: "stable" | "beta";
  pausedUntil: number | null;
}

export interface Entry {
  id: number;
  createdAt: string;
  text: string;
  rawText: string;
  app: string;
  durationMs: number;
  words: number;
  language: string;
  model: string;
  engine: string;
  processMs: number;
  hasAudio: boolean;
  favorite: boolean;
}

export interface Stats {
  wordsToday: number;
  wordsAvg: number;
  durationTodayMs: number;
  countToday: number;
  savedMinutesToday: number;
  streak: number;
  bestStreak: number;
  week: { label: string; words: number; today: boolean }[];
  weekTotal: number;
  totalCount: number;
}

export interface ModelInfo {
  id: string;
  name: string;
  sizeMb: number;
  desc: string;
  recommended: boolean;
  installed: boolean;
}

export interface ModelProgress {
  id: string;
  downloaded: number;
  total: number;
  done: boolean;
  error: string | null;
}

export type Phase = "hidden" | "idle" | "listening" | "processing" | "done" | "error";

export interface IslandPayload {
  phase: Phase;
  mode: "hold" | "toggle" | "locked" | null;
  text: string;
  elapsedMs: number;
  words: number;
  message: string;
  app: string;
}

export interface Backend {
  device: string;
  gpu: boolean;
  cpuFeatures: string;
  threads: number;
}

export interface AppInfo {
  version: string;
  platform: string;
  gpuBuild: boolean;
  gpuDevices: string[];
  localAvailable: boolean;
  dataDir: string;
  loadedModel: string | null;
  backend: Backend;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  url: string;
  notes: string;
  available: boolean;
}

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  listMics: () => invoke<string[]>("list_mics"),
  defaultMic: () => invoke<string | null>("default_mic"),
  micTestStart: (device: string | null) => invoke<string>("mic_test_start", { device }),
  micTestLevel: () => invoke<number>("mic_test_level"),
  micTestStop: () => invoke<void>("mic_test_stop"),
  modelsList: () => invoke<ModelInfo[]>("models_list"),
  modelDownloading: () => invoke<string | null>("model_downloading"),
  modelDownload: (id: string) => invoke<void>("model_download", { id }),
  modelCancel: () => invoke<void>("model_cancel"),
  modelDelete: (id: string) => invoke<void>("model_delete", { id }),
  historyList: (query: string, favorites: boolean, appFilter: string | null, limit = 200, offset = 0) =>
    invoke<Entry[]>("history_list", { query, favorites, appFilter, limit, offset }),
  historyApps: () => invoke<string[]>("history_apps"),
  toggleFavorite: (id: number) => invoke<boolean>("history_toggle_favorite", { id }),
  historyDelete: (id: number) => invoke<void>("history_delete", { id }),
  historyClear: () => invoke<void>("history_clear"),
  historyDropAudio: () => invoke<void>("history_drop_audio"),
  historyExport: (path: string, format: "json" | "md") => invoke<number>("history_export", { path, format }),
  historyAudio: (id: number) => invoke<string | null>("history_audio", { id }),
  stats: () => invoke<Stats>("stats"),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  reinsertText: (text: string) => invoke<void>("reinsert_text", { text }),
  dictationToggle: () => invoke<void>("dictation_toggle"),
  dictationCancel: () => invoke<void>("dictation_cancel"),
  islandState: () => invoke<IslandPayload>("island_state"),
  hotkeyCaptureStart: () => invoke<void>("hotkey_capture_start"),
  hotkeyCaptureCancel: () => invoke<void>("hotkey_capture_cancel"),
  appInfo: () => invoke<AppInfo>("app_info"),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  openLogs: () => invoke<void>("open_logs"),
  quit: () => invoke<void>("quit_app"),
};

export function on<T>(event: string, cb: (payload: T) => void): () => void {
  let un: UnlistenFn | null = null;
  let dead = false;
  listen<T>(event, (e) => cb(e.payload)).then((u) => {
    if (dead) u();
    else un = u;
  });
  return () => {
    dead = true;
    un?.();
  };
}

export const isTauri = () => "__TAURI_INTERNALS__" in window;
