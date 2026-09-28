//! Уведомления Windows (тосты) с кнопками «Отложить» / «Готово».

use tauri::AppHandle;

#[cfg(windows)]
pub fn show_reminder(app: &AppHandle, id: i64, title: &str, body: &str, firing: bool) {
    use tauri::Emitter;
    use tauri_winrt_notification::{Duration, Scenario, Sound, Toast};

    // Установщик прописывает ярлыку идентификатор приложения — тогда тост подписан «Маячок».
    // В dev-сборке ярлыка нет, и Windows такие тосты молча выкидывает — берём PowerShell.
    let app_id = if cfg!(debug_assertions) { Toast::POWERSHELL_APP_ID.to_string() } else { app.config().identifier.clone() };
    let app2 = app.clone();
    let mut t = Toast::new(&app_id).title(title).text1(body);
    if firing {
        // Висит на экране, пока не нажмёшь
        t = t
            .scenario(Scenario::Reminder)
            .sound(Some(Sound::Reminder))
            .add_button("Отложить на 10 мин", &format!("snooze:{id}"))
            .add_button("Готово", &format!("done:{id}"));
    } else {
        t = t.duration(Duration::Short).sound(None);
    }
    let res = t
        .on_activated(move |action| {
            let eng = crate::engine::state(&app2);
            match action.as_deref().and_then(|a| a.split_once(':')) {
                Some((act, rid)) => {
                    if let Ok(rid) = rid.parse::<i64>() {
                        if let Err(e) = eng.reminder_action(rid, act) {
                            log::warn!("Маячок из уведомления: {e}");
                        }
                    }
                }
                None => {
                    // Клик по самому уведомлению — открыть «Маячки»
                    crate::show_main(&app2);
                    let _ = app2.emit("navigate", "reminders");
                }
            }
            Ok(())
        })
        .show();
    if let Err(e) = res {
        log::warn!("Уведомление Windows не показалось: {e}");
    }
}

#[cfg(not(windows))]
pub fn show_reminder(_app: &AppHandle, id: i64, title: &str, body: &str, _firing: bool) {
    log::info!("[уведомление] #{id} {title} — {body}");
}
