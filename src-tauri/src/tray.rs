//! Иконка и меню в трее.

use crate::engine;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Wry};

const ID: &str = "main";
static ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
static ICON_REC: &[u8] = include_bytes!("../icons/tray-rec.png");

fn keys_label(keys: &[String]) -> String {
    keys.join(" + ")
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let eng = engine::state(app);
    let s = eng.settings();
    let recording = eng.is_recording();
    let status = if s.is_paused() {
        "На паузе".to_string()
    } else if recording {
        "Идёт запись…".to_string()
    } else {
        format!("Готов · {}", keys_label(&s.hotkeys.dictate))
    };

    let status_item = MenuItem::with_id(app, "status", status, false, None::<&str>)?;
    let dictate = MenuItem::with_id(
        app,
        "dictate",
        if recording { "Остановить диктовку" } else { "Начать диктовку" },
        !s.is_paused(),
        None::<&str>,
    )?;
    let last = eng.history.last();
    let copy_last = MenuItem::with_id(
        app,
        "copy-last",
        match &last {
            Some(e) => {
                let preview: String = e.text.chars().take(28).collect();
                format!("Скопировать: «{preview}{}»", if e.text.chars().count() > 28 { "…" } else { "" })
            }
            None => "Скопировать последнее".into(),
        },
        last.is_some(),
        None::<&str>,
    )?;
    let open = MenuItem::with_id(app, "open", "Открыть Маячок", true, None::<&str>)?;

    let mut mic_items: Vec<CheckMenuItem<Wry>> = vec![CheckMenuItem::with_id(
        app,
        "mic::",
        "Системный по умолчанию",
        true,
        s.mic_device.is_none(),
        None::<&str>,
    )?];
    for name in crate::audio::list_input_devices() {
        let checked = s.mic_device.as_deref() == Some(name.as_str());
        mic_items.push(CheckMenuItem::with_id(app, format!("mic::{name}"), &name, true, checked, None::<&str>)?);
    }
    let mic_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = mic_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
    let mic = Submenu::with_id_and_items(app, "mic", "Микрофон", true, &mic_refs)?;

    let langs = [("ru", "Русский"), ("en", "English"), ("uk", "Українська"), ("auto", "Автоопределение")];
    let mut lang_items = vec![];
    for (code, label) in langs {
        let checked = if code == "auto" { s.auto_detect } else { !s.auto_detect && s.language == code };
        lang_items.push(CheckMenuItem::with_id(app, format!("lang::{code}"), label, true, checked, None::<&str>)?);
    }
    let lang_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = lang_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
    let lang = Submenu::with_id_and_items(app, "lang", "Язык", true, &lang_refs)?;

    let pause = MenuItem::with_id(
        app,
        "pause",
        if s.is_paused() { "Снять паузу" } else { "Пауза на час" },
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выйти", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);

    Menu::with_items(
        app,
        &[
            &status_item,
            &sep()?,
            &dictate,
            &copy_last,
            &open,
            &sep()?,
            &mic,
            &lang,
            &pause,
            &sep()?,
            &settings,
            &quit,
        ],
    )
}

fn update_settings(app: &AppHandle, f: impl FnOnce(&mut crate::settings::Settings)) {
    let eng = engine::state(app);
    let s = {
        let mut s = eng.settings.write();
        f(&mut s);
        let _ = s.save(&eng.dirs.config);
        s.clone()
    };
    let _ = app.emit("settings-changed", &s);
    refresh(app);
}

fn on_menu(app: &AppHandle, id: &str) {
    let eng = engine::state(app);
    match id {
        "dictate" => {
            // Не в главном потоке: открытие микрофона может занять пару секунд
            std::thread::spawn(move || {
                if eng.is_recording() {
                    eng.stop();
                } else {
                    eng.start(engine::Mode::Locked, false);
                }
            });
        }
        "copy-last" => {
            if let Some(e) = eng.history.last() {
                let _ = crate::platform::copy_text(&e.text);
            }
        }
        "open" => crate::show_main(app),
        "settings" => {
            crate::show_main(app);
            let _ = app.emit("navigate", "settings");
        }
        "pause" => update_settings(app, |s| {
            s.paused_until = if s.is_paused() { None } else { Some(chrono::Utc::now().timestamp_millis() + 3_600_000) };
        }),
        "quit" => app.exit(0),
        other => {
            if let Some(name) = other.strip_prefix("mic::") {
                let name = name.to_string();
                update_settings(app, |s| s.mic_device = if name.is_empty() { None } else { Some(name) });
            } else if let Some(code) = other.strip_prefix("lang::") {
                let code = code.to_string();
                update_settings(app, |s| {
                    if code == "auto" {
                        s.auto_detect = true;
                    } else {
                        s.auto_detect = false;
                        s.language = code;
                    }
                });
            }
        }
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(ID)
        .icon(Image::from_bytes(ICON_IDLE)?)
        .tooltip("Маячок")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                crate::show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Перестраивает меню (статус, последняя запись, галочки)
pub fn refresh(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let (Some(tray), Ok(menu)) = (app2.tray_by_id(ID), build_menu(&app2)) {
            let _ = tray.set_menu(Some(menu));
        }
    });
}

pub fn set_recording(app: &AppHandle, on: bool) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = app2.tray_by_id(ID) {
            if let Ok(img) = Image::from_bytes(if on { ICON_REC } else { ICON_IDLE }) {
                let _ = tray.set_icon(Some(img));
            }
            let _ = tray.set_tooltip(Some(if on { "Маячок — запись" } else { "Маячок" }));
        }
    });
    refresh(app);
}
