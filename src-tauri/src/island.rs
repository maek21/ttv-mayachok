//! Окно-остров: прозрачное, поверх всех, не кликается и не забирает фокус.

use crate::settings::Settings;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "island";
const W: f64 = 760.0;
const H: f64 = 360.0;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html#/island".into()))
        .title("Маячок — остров")
        .inner_size(W, H)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;
    crate::platform::make_overlay(&w);
    Ok(())
}

fn place(app: &AppHandle, s: &Settings) {
    let Some(w) = app.get_webview_window(LABEL) else { return };
    let cursor = app.cursor_position().ok();
    let monitor = cursor
        .and_then(|c| app.monitor_from_point(c.x, c.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(m) = monitor else { return };
    let scale = m.scale_factor();
    let area = m.work_area();
    let (ax, ay) = (area.position.x as f64, area.position.y as f64);
    let (aw, ah) = (area.size.width as f64, area.size.height as f64);
    let (pw, ph) = (W * scale, H * scale);
    let (x, y) = match s.island.position.as_str() {
        "bottom" => (ax + (aw - pw) / 2.0, ay + ah - ph - 12.0 * scale),
        "cursor" => {
            let c = cursor.map(|c| (c.x, c.y)).unwrap_or((ax + aw / 2.0, ay + ah / 2.0));
            let x = (c.0 - pw / 2.0).clamp(ax, ax + aw - pw);
            let y = (c.1 + 24.0 * scale).clamp(ay, ay + ah - ph);
            (x, y)
        }
        _ => (ax + (aw - pw) / 2.0, ay + 10.0 * scale),
    };
    let _ = w.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

pub fn show(app: &AppHandle, s: &Settings) {
    let Some(w) = app.get_webview_window(LABEL) else { return };
    if !w.is_visible().unwrap_or(false) {
        place(app, s);
    }
    crate::platform::show_no_activate(&w);
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        crate::platform::hide(&w);
    }
}
