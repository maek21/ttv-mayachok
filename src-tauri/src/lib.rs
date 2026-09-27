mod audio;
mod commands;
mod engine;
mod history;
mod hotkey;
mod island;
mod models;
mod platform;
mod postprocess;
mod settings;
mod transcribe;
mod tray;

use engine::{Dirs, Engine};
use std::sync::Arc;
use tauri::{AppHandle, Manager, WindowEvent};

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let paths = app.path();
            let dirs = Dirs {
                config: paths.app_config_dir()?,
                data: paths.app_data_dir()?,
                models: paths.app_data_dir()?.join("models"),
                logs: paths.app_log_dir()?,
            };
            let eng = Arc::new(Engine::new(handle.clone(), dirs).map_err(|e| e.to_string())?);
            app.manage(eng.clone());
            let s = eng.settings();

            island::create(&handle)?;
            tray::create(&handle)?;

            // Хоткеи → движок
            let (tx, rx) = std::sync::mpsc::channel();
            hotkey::start(tx, &s.hotkeys);
            let eng2 = eng.clone();
            std::thread::Builder::new()
                .name("hotkey-events".into())
                .spawn(move || {
                    for ev in rx {
                        eng2.on_hotkey(ev);
                    }
                })?;

            // Автозапуск в системе должен совпадать с настройкой (по умолчанию включён)
            {
                use tauri_plugin_autostart::ManagerExt;
                let al = handle.autolaunch();
                if al.is_enabled().unwrap_or(false) != s.autostart {
                    let r = if s.autostart { al.enable() } else { al.disable() };
                    if let Err(e) = r {
                        log::warn!("Автозапуск: {e}");
                    }
                }
            }

            eng.warm_up();
            if !s.island.hide_idle {
                eng.emit(engine::IslandPayload { phase: "idle".into(), ..Default::default() });
            }

            let minimized = std::env::args().any(|a| a == "--minimized");
            if !minimized || !s.onboarded {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let eng = engine::state(window.app_handle());
                if eng.settings().close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::list_mics,
            commands::default_mic,
            commands::mic_test_start,
            commands::mic_test_level,
            commands::mic_test_stop,
            commands::models_list,
            commands::model_downloading,
            commands::model_download,
            commands::model_cancel,
            commands::model_delete,
            commands::history_list,
            commands::history_apps,
            commands::history_toggle_favorite,
            commands::history_delete,
            commands::history_clear,
            commands::history_drop_audio,
            commands::history_export,
            commands::history_audio,
            commands::stats,
            commands::copy_text,
            commands::reinsert_text,
            commands::dictation_toggle,
            commands::dictation_cancel,
            commands::island_state,
            commands::hotkey_capture_start,
            commands::hotkey_capture_cancel,
            commands::app_info,
            commands::check_update,
            commands::open_logs,
            commands::quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить Маячок");
}
