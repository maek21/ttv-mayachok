#![cfg_attr(not(windows), allow(dead_code))]

//! Всё, что завязано на WinAPI: вставка, активное приложение, окно-остров без фокуса, звуки.
//! На других ОС — заглушки, чтобы проект собирался и проверялся.

#[derive(Debug, Clone, Default)]
pub struct ForegroundApp {
    /// Человеческое имя: «Telegram», «VS Code»…
    pub name: String,
    /// Имя exe без расширения
    pub exe: String,
    /// Это наше собственное окно
    #[allow(dead_code)]
    pub is_self: bool,
}

pub fn pretty_app_name(exe: &str) -> String {
    let lower = exe.to_lowercase();
    let known = [
        ("telegram", "Telegram"),
        ("code", "VS Code"),
        ("cursor", "Cursor"),
        ("chrome", "Chrome"),
        ("msedge", "Edge"),
        ("firefox", "Firefox"),
        ("browser", "Яндекс Браузер"),
        ("opera", "Opera"),
        ("notion", "Notion"),
        ("obsidian", "Obsidian"),
        ("slack", "Slack"),
        ("discord", "Discord"),
        ("winword", "Word"),
        ("excel", "Excel"),
        ("powerpnt", "PowerPoint"),
        ("outlook", "Outlook"),
        ("olk", "Outlook"),
        ("ms-teams", "Teams"),
        ("teams", "Teams"),
        ("notepad", "Блокнот"),
        ("windowsterminal", "Terminal"),
        ("explorer", "Проводник"),
        ("figma", "Figma"),
        ("whatsapp", "WhatsApp"),
        ("zoom", "Zoom"),
        ("idea64", "IntelliJ IDEA"),
        ("pycharm64", "PyCharm"),
        ("mayachok", "Маячок"),
    ];
    known
        .iter()
        .find(|(k, _)| lower == *k)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| {
            let mut c = exe.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::OnceLock;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    pub fn foreground_app() -> ForegroundApp {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return ForegroundApp::default();
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let is_self = pid == std::process::id();
            let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return ForegroundApp { is_self, ..Default::default() };
            };
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(h);
            if !ok {
                return ForegroundApp { is_self, ..Default::default() };
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            let exe = std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            ForegroundApp { name: pretty_app_name(&exe), exe, is_self }
        }
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn held(vk: VIRTUAL_KEY) -> bool {
        unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
    }

    /// Ctrl+V в активное окно. Ждём, пока человек отпустит Win/Alt/Shift, иначе получится другое сочетание.
    pub fn send_paste() {
        let t = std::time::Instant::now();
        while t.elapsed().as_millis() < 800
            && (held(VK_LWIN) || held(VK_RWIN) || held(VK_MENU) || held(VK_SHIFT))
        {
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let inputs = [key(VK_CONTROL, false), key(VK_V, false), key(VK_V, true), key(VK_CONTROL, true)];
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    /// «Пустая» клавиша + отпускание Win/Alt, чтобы не открылось меню «Пуск» / строка меню.
    pub fn send_mask_and_release(release_vk: u16) {
        let mask = VIRTUAL_KEY(0xE8);
        let inputs = [key(mask, false), key(mask, true), key(VIRTUAL_KEY(release_vk), true)];
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn hwnd_of(w: &tauri::WebviewWindow) -> Option<HWND> {
        w.hwnd().ok().map(|h| HWND(h.0 as _))
    }

    /// Остров никогда не забирает фокус: иначе текст вставится не туда.
    pub fn make_overlay(w: &tauri::WebviewWindow) {
        if let Some(h) = hwnd_of(w) {
            unsafe {
                let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
                SetWindowLongPtrW(h, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize);
            }
        }
        let _ = w.set_ignore_cursor_events(true);
    }

    pub fn show_no_activate(w: &tauri::WebviewWindow) {
        if let Some(h) = hwnd_of(w) {
            unsafe {
                let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(
                    h,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        } else {
            let _ = w.show();
        }
    }

    pub fn hide(w: &tauri::WebviewWindow) {
        if let Some(h) = hwnd_of(w) {
            unsafe {
                let _ = ShowWindow(h, SW_HIDE);
            }
        } else {
            let _ = w.hide();
        }
    }

    fn tone(freqs: &[(f32, f32)]) -> Vec<u8> {
        let rate = 22_050u32;
        let mut samples: Vec<f32> = vec![];
        for (f, dur) in freqs {
            let n = (rate as f32 * dur) as usize;
            for i in 0..n {
                let t = i as f32 / rate as f32;
                let env = (1.0 - i as f32 / n as f32).powf(2.0) * (i as f32 / 200.0).min(1.0);
                samples.push((2.0 * std::f32::consts::PI * f * t).sin() * 0.25 * env);
            }
        }
        let spec = hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut cur = std::io::Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(&mut cur, spec).expect("wav");
            for s in samples {
                let _ = w.write_sample((s * i16::MAX as f32) as i16);
            }
            let _ = w.finalize();
        }
        cur.into_inner()
    }

    pub fn play_sound(start: bool) {
        static START: OnceLock<Vec<u8>> = OnceLock::new();
        static STOP: OnceLock<Vec<u8>> = OnceLock::new();
        let data = if start {
            START.get_or_init(|| tone(&[(880.0, 0.07), (1320.0, 0.09)]))
        } else {
            STOP.get_or_init(|| tone(&[(1320.0, 0.07), (880.0, 0.09)]))
        };
        unsafe {
            let _ = PlaySoundW(PCWSTR(data.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn foreground_app() -> ForegroundApp {
        ForegroundApp::default()
    }
    pub fn send_paste() {
        log::warn!("Автовставка реализована только для Windows");
    }
    pub fn send_mask_and_release(_vk: u16) {}
    pub fn make_overlay(w: &tauri::WebviewWindow) {
        let _ = w.set_ignore_cursor_events(true);
    }
    pub fn show_no_activate(w: &tauri::WebviewWindow) {
        let _ = w.show();
    }
    pub fn hide(w: &tauri::WebviewWindow) {
        let _ = w.hide();
    }
    pub fn play_sound(_start: bool) {}
}

pub use imp::*;

/// Буфер обмена: кладём текст, вставляем, по желанию возвращаем прежнее содержимое.
pub fn paste_text(text: &str, restore: bool) -> Result<(), String> {
    enum Prev {
        Text(String),
        Image(arboard::ImageData<'static>),
        None,
    }
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Буфер обмена недоступен: {e}"))?;
    let prev = if restore {
        match cb.get_text() {
            Ok(t) => Prev::Text(t),
            Err(_) => cb.get_image().map(|i| Prev::Image(i.to_owned_img())).unwrap_or(Prev::None),
        }
    } else {
        Prev::None
    };
    cb.set_text(text.to_string()).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(40));
    send_paste();
    if restore {
        // Даём приложению прочитать буфер, потом возвращаем как было
        std::thread::sleep(std::time::Duration::from_millis(450));
        match prev {
            Prev::Text(t) => {
                let _ = cb.set_text(t);
            }
            Prev::Image(i) => {
                let _ = cb.set_image(i);
            }
            Prev::None => {}
        }
    }
    Ok(())
}

pub fn copy_text(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.to_string()))
        .map_err(|e| e.to_string())
}
