//! Команды, которые вызывает интерфейс.

use crate::engine::{self, IslandPayload, Mode};
use crate::history::{Entry, Stats};
use crate::models::{self, ModelInfo, Progress};
use crate::settings::Settings;
use crate::{hotkey, platform, tray};
use base64::Engine as _;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

type R<T> = Result<T, String>;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    engine::state(&app).settings()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> R<Settings> {
    let eng = engine::state(&app);
    let prev = eng.settings();
    settings.save(&eng.dirs.config)?;
    *eng.settings.write() = settings.clone();

    if prev.hotkeys.dictate != settings.hotkeys.dictate
        || prev.hotkeys.hands_free != settings.hotkeys.hands_free
        || prev.hotkeys.cancel != settings.hotkeys.cancel
        || prev.hotkeys.paste_last != settings.hotkeys.paste_last
        || prev.hotkeys.open_app != settings.hotkeys.open_app
    {
        hotkey::update(&settings.hotkeys);
    }
    if prev.autostart != settings.autostart {
        let al = app.autolaunch();
        let r = if settings.autostart { al.enable() } else { al.disable() };
        if let Err(e) = r {
            log::warn!("Автозапуск: {e}");
        }
    }
    if prev.history_days != settings.history_days {
        eng.history.apply_retention(settings.history_days);
    }
    if prev.local_model != settings.local_model || prev.engine != settings.engine {
        eng.local.unload();
        eng.warm_up();
    }
    if prev.island.hide_idle != settings.island.hide_idle && !eng.is_recording() {
        eng.emit(IslandPayload { phase: if settings.island.hide_idle { "hidden" } else { "idle" }.into(), ..Default::default() });
    }
    let _ = app.emit("settings-changed", &settings);
    tray::refresh(&app);
    Ok(settings)
}

#[tauri::command]
pub fn list_mics() -> Vec<String> {
    crate::audio::list_input_devices()
}

#[tauri::command]
pub fn default_mic() -> Option<String> {
    crate::audio::default_input_name()
}

#[tauri::command]
pub fn mic_test_start(app: AppHandle, device: Option<String>) -> R<String> {
    engine::state(&app).start_mic_test(device)
}

#[tauri::command]
pub fn mic_test_level(app: AppHandle) -> f32 {
    engine::state(&app).mic_test_level()
}

#[tauri::command]
pub fn mic_test_stop(app: AppHandle) {
    engine::state(&app).stop_mic_test()
}

#[tauri::command]
pub fn models_list(app: AppHandle) -> Vec<ModelInfo> {
    models::list(&engine::state(&app).dirs.models)
}

#[tauri::command]
pub fn model_downloading(app: AppHandle) -> Option<String> {
    engine::state(&app).downloading.lock().clone()
}

#[tauri::command]
pub fn model_download(app: AppHandle, id: String) -> R<()> {
    let eng = engine::state(&app);
    {
        let mut d = eng.downloading.lock();
        if d.is_some() {
            return Err("Уже качаю другую модель".into());
        }
        *d = Some(id.clone());
    }
    eng.download_cancel.store(false, Ordering::SeqCst);
    let app2 = app.clone();
    std::thread::spawn(move || {
        let eng = engine::state(&app2);
        let res = models::download(&eng.dirs.models, &id, eng.download_cancel.clone(), |p| {
            let _ = app2.emit("model-progress", &p);
        });
        *eng.downloading.lock() = None;
        match res {
            Ok(()) => {
                eng.warm_up();
            }
            Err(e) => {
                let _ = app2.emit("model-progress", Progress { id, downloaded: 0, total: 0, done: false, error: Some(e) });
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn model_cancel(app: AppHandle) {
    engine::state(&app).download_cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn model_delete(app: AppHandle, id: String) -> R<()> {
    let eng = engine::state(&app);
    if eng.local.loaded_model().as_deref() == Some(id.as_str()) {
        eng.local.unload();
    }
    models::delete(&eng.dirs.models, &id)
}

#[tauri::command]
pub fn history_list(app: AppHandle, query: String, favorites: bool, app_filter: Option<String>, limit: i64, offset: i64) -> R<Vec<Entry>> {
    engine::state(&app).history.list(&query, favorites, app_filter.as_deref(), limit, offset)
}

#[tauri::command]
pub fn history_apps(app: AppHandle) -> R<Vec<String>> {
    engine::state(&app).history.apps()
}

#[tauri::command]
pub fn history_toggle_favorite(app: AppHandle, id: i64) -> R<bool> {
    engine::state(&app).history.toggle_favorite(id)
}

#[tauri::command]
pub fn history_delete(app: AppHandle, id: i64) -> R<()> {
    engine::state(&app).history.delete(id)?;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub fn history_clear(app: AppHandle) -> R<()> {
    engine::state(&app).history.clear()?;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub fn history_drop_audio(app: AppHandle) {
    engine::state(&app).history.drop_audio();
}

#[tauri::command]
pub fn history_export(app: AppHandle, path: String, format: String) -> R<usize> {
    let all = engine::state(&app).history.all()?;
    let data = if format == "md" {
        let mut out = String::from("# Маячок — история\n\n");
        for e in &all {
            let when = chrono::DateTime::parse_from_rfc3339(&e.created_at)
                .map(|d| d.with_timezone(&chrono::Local).format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_default();
            out.push_str(&format!("## {when} · {}\n\n{}\n\n", if e.app.is_empty() { "—" } else { &e.app }, e.text));
        }
        out
    } else {
        serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?
    };
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(all.len())
}

/// Аудио записи как data: URL (записи короткие, так проще, чем раздавать файлы)
#[tauri::command]
pub fn history_audio(app: AppHandle, id: i64) -> R<Option<String>> {
    let Some(p) = engine::state(&app).history.audio_path(id) else { return Ok(None) };
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    Ok(Some(format!("data:audio/wav;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))))
}

#[tauri::command]
pub fn stats(app: AppHandle) -> R<Stats> {
    engine::state(&app).history.stats()
}

#[tauri::command]
pub fn copy_text(text: String) -> R<()> {
    platform::copy_text(&text)
}

/// «Вставить снова»: прячем окно, фокус возвращается в прошлое приложение, вставляем туда.
#[tauri::command]
pub fn reinsert_text(app: AppHandle, text: String) -> R<()> {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.minimize();
    }
    let restore = engine::state(&app).settings().clipboard_mode == "restore";
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(350));
        if let Err(e) = platform::paste_text(&text, restore) {
            log::error!("Повторная вставка: {e}");
        }
    });
    Ok(())
}

#[tauri::command]
pub fn dictation_toggle(app: AppHandle) {
    let eng = engine::state(&app);
    if eng.is_recording() {
        eng.stop();
    } else {
        eng.start(Mode::Locked, true);
    }
}

#[tauri::command]
pub fn dictation_cancel(app: AppHandle) {
    engine::state(&app).cancel(true);
}

#[tauri::command]
pub fn island_state(app: AppHandle) -> IslandPayload {
    engine::state(&app).last_payload.lock().clone()
}

#[tauri::command]
pub fn hotkey_capture_start() {
    hotkey::begin_capture();
}

#[tauri::command]
pub fn hotkey_capture_cancel() {
    hotkey::cancel_capture();
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    platform: String,
    gpu: bool,
    local_available: bool,
    data_dir: String,
    loaded_model: Option<String>,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    let eng = engine::state(&app);
    AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        gpu: cfg!(any(feature = "cuda", feature = "vulkan")),
        local_available: cfg!(feature = "local-whisper"),
        data_dir: eng.dirs.data.to_string_lossy().into(),
        loaded_model: eng.local.loaded_model(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: String,
    latest: String,
    url: String,
    notes: String,
    available: bool,
}

fn parse_ver(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split(['.', '-']).filter_map(|p| p.parse().ok()).collect()
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> R<UpdateInfo> {
    let beta = engine::state(&app).settings().update_channel == "beta";
    let url = "https://api.github.com/repos/maek21/ttv-mayachok/releases";
    let client = reqwest::Client::builder()
        .user_agent(concat!("Mayachok/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| format!("Нет соединения: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub ответил {}", resp.status()));
    }
    let list: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
    let rel = list
        .iter()
        .find(|r| !r["draft"].as_bool().unwrap_or(false) && (beta || !r["prerelease"].as_bool().unwrap_or(false)));
    let current = env!("CARGO_PKG_VERSION").to_string();
    let Some(rel) = rel else {
        return Ok(UpdateInfo { current: current.clone(), latest: current, url: String::new(), notes: String::new(), available: false });
    };
    let latest = rel["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string();
    Ok(UpdateInfo {
        available: parse_ver(&latest) > parse_ver(&current),
        current,
        latest,
        url: rel["html_url"].as_str().unwrap_or_default().into(),
        notes: rel["body"].as_str().unwrap_or_default().chars().take(600).collect(),
    })
}

#[tauri::command]
pub fn open_logs(app: AppHandle) -> R<()> {
    let dir = engine::state(&app).dirs.logs.clone();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    tauri_plugin_opener::open_path(&dir, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
