#![cfg_attr(not(windows), allow(dead_code))]

//! Глобальные сочетания через низкоуровневый хук клавиатуры (WH_KEYBOARD_LL).
//! Обычные глобальные хоткеи не умеют «зажал — отпустил» и сочетания из одних модификаторов
//! вроде Ctrl + Win, поэтому слушаем клавиатуру сами.

use crate::settings::Hotkeys;
use std::collections::BTreeSet;
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    DictateDown,
    DictateUp,
    /// Сочетание сорвалось (нажали что-то ещё, например Ctrl+Win+→) — запись выбросить
    DictateAbort,
    HandsFree,
    Cancel,
    PasteLast,
    OpenApp,
    Captured(Vec<String>),
}

pub const MODIFIERS: [&str; 4] = ["Ctrl", "Alt", "Shift", "Win"];

pub fn is_modifier(k: &str) -> bool {
    MODIFIERS.contains(&k)
}

/// Порядок для отображения: модификаторы, потом остальное
pub fn sort_keys(keys: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = keys.into_iter().collect();
    v.sort_by_key(|k| (MODIFIERS.iter().position(|m| m == k).unwrap_or(10), k.clone()));
    v.dedup();
    v
}

#[derive(Default, Clone)]
struct Combos {
    dictate: BTreeSet<String>,
    hands_free: BTreeSet<String>,
    cancel: BTreeSet<String>,
    paste_last: BTreeSet<String>,
    open_app: BTreeSet<String>,
}

impl From<&Hotkeys> for Combos {
    fn from(h: &Hotkeys) -> Self {
        let set = |v: &Vec<String>| v.iter().cloned().collect::<BTreeSet<_>>();
        Self {
            dictate: set(&h.dictate),
            hands_free: set(&h.hands_free),
            cancel: set(&h.cancel),
            paste_last: set(&h.paste_last),
            open_app: set(&h.open_app),
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::OnceLock;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    struct State {
        combos: Combos,
        pressed: BTreeSet<String>,
        /// клавиши, чьё нажатие мы проглотили — проглатываем и отпускание
        swallowed: BTreeSet<String>,
        dictate_active: bool,
        recording: bool,
        mask_pending: bool,
        capture: Option<BTreeSet<String>>,
        tx: Option<Sender<HotkeyEvent>>,
    }

    static STATE: OnceLock<Mutex<State>> = OnceLock::new();

    fn state() -> &'static Mutex<State> {
        STATE.get_or_init(|| {
            Mutex::new(State {
                combos: Combos::default(),
                pressed: BTreeSet::new(),
                swallowed: BTreeSet::new(),
                dictate_active: false,
                recording: false,
                mask_pending: false,
                capture: None,
                tx: None,
            })
        })
    }

    fn name_of(vk: u32) -> String {
        let vk16 = vk as u16;
        match VIRTUAL_KEY(vk16) {
            VK_LCONTROL | VK_RCONTROL | VK_CONTROL => "Ctrl".into(),
            VK_LMENU | VK_RMENU | VK_MENU => "Alt".into(),
            VK_LSHIFT | VK_RSHIFT | VK_SHIFT => "Shift".into(),
            VK_LWIN | VK_RWIN => "Win".into(),
            VK_SPACE => "Space".into(),
            VK_ESCAPE => "Esc".into(),
            VK_RETURN => "Enter".into(),
            VK_TAB => "Tab".into(),
            VK_BACK => "Backspace".into(),
            VK_CAPITAL => "CapsLock".into(),
            VK_OEM_3 => "`".into(),
            _ if (0x41..=0x5A).contains(&vk) || (0x30..=0x39).contains(&vk) => (vk as u8 as char).to_string(),
            _ if (0x70..=0x87).contains(&vk) => format!("F{}", vk - 0x6F),
            _ => format!("VK{vk:02X}"),
        }
    }

    fn vks_of(name: &str) -> Vec<VIRTUAL_KEY> {
        match name {
            "Ctrl" => vec![VK_LCONTROL, VK_RCONTROL],
            "Alt" => vec![VK_LMENU, VK_RMENU],
            "Shift" => vec![VK_LSHIFT, VK_RSHIFT],
            "Win" => vec![VK_LWIN, VK_RWIN],
            _ => vec![],
        }
    }

    /// Выкидываем «залипшие» модификаторы, чьё отпускание хук мог пропустить (UAC, блокировка экрана)
    fn prune(st: &mut State, current: &str) {
        st.pressed.retain(|k| {
            if k == current {
                return true;
            }
            let vks = vks_of(k);
            if vks.is_empty() {
                return true;
            }
            vks.iter().any(|v| unsafe { (GetAsyncKeyState(v.0 as i32) as u16 & 0x8000) != 0 })
        });
    }

    fn send(st: &State, e: HotkeyEvent) {
        if let Some(tx) = &st.tx {
            let _ = tx.send(e);
        }
    }

    fn needs_mask(set: &BTreeSet<String>) -> bool {
        set.contains("Win") || set.contains("Alt")
    }

    /// true — проглотить событие
    fn handle(vk: u32, down: bool) -> bool {
        let name = name_of(vk);
        let mut st = state().lock();

        if let Some(cap) = st.capture.as_mut() {
            if down {
                cap.insert(name.clone());
                st.pressed.insert(name);
            } else {
                st.pressed.remove(&name);
                if st.pressed.is_empty() {
                    let keys = sort_keys(st.capture.take().unwrap_or_default());
                    send(&st, HotkeyEvent::Captured(keys));
                }
            }
            return true;
        }

        if down {
            prune(&mut st, &name);
            let repeat = !st.pressed.insert(name.clone());
            if repeat {
                return st.swallowed.contains(&name);
            }
            let combos = st.combos.clone();
            let swallow_key = !is_modifier(&name);

            if !combos.hands_free.is_empty() && st.pressed == combos.hands_free {
                st.mask_pending |= needs_mask(&combos.hands_free);
                st.dictate_active = false;
                send(&st, HotkeyEvent::HandsFree);
                if swallow_key {
                    st.swallowed.insert(name);
                }
                return swallow_key;
            }
            if !combos.dictate.is_empty() && st.pressed == combos.dictate && !st.dictate_active {
                st.dictate_active = true;
                st.mask_pending |= needs_mask(&combos.dictate);
                send(&st, HotkeyEvent::DictateDown);
                if swallow_key {
                    st.swallowed.insert(name);
                }
                return swallow_key;
            }
            if st.dictate_active && !combos.dictate.contains(&name) && !combos.hands_free.contains(&name) {
                // Это было другое сочетание (Ctrl+Win+D и т.п.) — отдаём его системе
                st.dictate_active = false;
                st.mask_pending = false;
                send(&st, HotkeyEvent::DictateAbort);
                return false;
            }
            if st.recording && !combos.cancel.is_empty() && st.pressed == combos.cancel {
                send(&st, HotkeyEvent::Cancel);
                st.swallowed.insert(name);
                return true;
            }
            if !combos.paste_last.is_empty() && st.pressed == combos.paste_last {
                st.mask_pending |= needs_mask(&combos.paste_last);
                send(&st, HotkeyEvent::PasteLast);
                st.swallowed.insert(name);
                return swallow_key;
            }
            if !combos.open_app.is_empty() && st.pressed == combos.open_app {
                st.mask_pending |= needs_mask(&combos.open_app);
                send(&st, HotkeyEvent::OpenApp);
                st.swallowed.insert(name);
                return swallow_key;
            }
            false
        } else {
            st.pressed.remove(&name);
            if st.dictate_active && st.combos.dictate.contains(&name) {
                st.dictate_active = false;
                send(&st, HotkeyEvent::DictateUp);
            }
            let mask = st.mask_pending && (name == "Win" || name == "Alt");
            if mask {
                st.mask_pending = false;
            }
            let swallow = st.swallowed.remove(&name);
            drop(st);
            if mask {
                // Глотаем настоящее отпускание и шлём «пустую» клавишу + отпускание заново:
                // так Windows видит, что Win нажимали не в одиночку, и «Пуск» не открывается
                crate::platform::send_mask_and_release(vk as u16);
                return true;
            }
            swallow
        }
    }

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            // Свои же синтетические нажатия (Ctrl+V, маска) пропускаем
            if kb.flags.0 & LLKHF_INJECTED.0 == 0 {
                let msg = wparam.0 as u32;
                let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
                if (down || up) && handle(kb.vkCode, down) {
                    return LRESULT(1);
                }
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    pub fn start(tx: Sender<HotkeyEvent>, hotkeys: &Hotkeys) {
        {
            let mut st = state().lock();
            st.tx = Some(tx);
            st.combos = hotkeys.into();
        }
        std::thread::Builder::new()
            .name("keyboard-hook".into())
            .spawn(|| unsafe {
                // Windows снимает хук, если он отвечает дольше ~300 мс. Пока whisper грузит
                // все ядра, обычный приоритет может не успеть — поэтому поток хука самый срочный
                let _ = windows::Win32::System::Threading::SetThreadPriority(
                    windows::Win32::System::Threading::GetCurrentThread(),
                    windows::Win32::System::Threading::THREAD_PRIORITY_TIME_CRITICAL,
                );
                match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
                    Ok(_hook) => {
                        log::info!("Хук клавиатуры установлен");
                        let mut msg = MSG::default();
                        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                    Err(e) => log::error!("Не удалось поставить хук клавиатуры: {e}"),
                }
            })
            .expect("hook thread");
    }

    pub fn update(hotkeys: &Hotkeys) {
        let mut st = state().lock();
        st.combos = hotkeys.into();
        st.dictate_active = false;
    }

    pub fn set_recording(on: bool) {
        state().lock().recording = on;
    }

    pub fn begin_capture() {
        let mut st = state().lock();
        st.capture = Some(BTreeSet::new());
        st.pressed.clear();
    }

    pub fn cancel_capture() {
        state().lock().capture = None;
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn start(_tx: Sender<HotkeyEvent>, _hotkeys: &Hotkeys) {
        log::warn!("Глобальные сочетания реализованы только для Windows");
    }
    pub fn update(_hotkeys: &Hotkeys) {}
    pub fn set_recording(_on: bool) {}
    pub fn begin_capture() {}
    pub fn cancel_capture() {}
}

pub use imp::*;
