//! Сердце Маячка: запись → распознавание → чистка → вставка → история.

use crate::audio::{self, Capture};
use crate::history::{count_words, History, NewEntry};
use crate::platform::{self, ForegroundApp};
use crate::settings::Settings;
use crate::transcribe::{self, local::LocalEngine};
use crate::reminders::{self as rem, IslandReminders, Reminders};
use crate::{hotkey, island, models, tray};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const MAX_RECORDING: Duration = Duration::from_secs(10 * 60);
const MIN_RECORDING: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Hold,
    Toggle,
    Locked,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IslandPayload {
    /// hidden | idle | listening | processing | done | error
    pub phase: String,
    pub mode: Option<Mode>,
    pub text: String,
    pub elapsed_ms: u64,
    pub words: i64,
    pub message: String,
    pub app: String,
}

pub struct Dirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub models: PathBuf,
    pub logs: PathBuf,
}

struct Session {
    id: u64,
    capture: Capture,
    started: Instant,
    mode: Mode,
    target: ForegroundApp,
    from_ui: bool,
    partial: Arc<Mutex<String>>,
    /// Прерывает живой предпросмотр, как только запись закончилась
    abort: Arc<AtomicBool>,
}

pub struct Engine {
    pub app: AppHandle,
    pub dirs: Dirs,
    pub settings: RwLock<Settings>,
    pub history: History,
    pub reminders: Reminders,
    /// Что из маячков сейчас на острове
    pub rem_island: Mutex<IslandReminders>,
    /// Прямоугольник острова в окне (CSS px) — по нему включаем клики мышью
    pub hit_rect: Mutex<Option<[f64; 4]>>,
    pub local: LocalEngine,
    session: Mutex<Option<Session>>,
    busy: AtomicBool,
    seq: AtomicU64,
    pub last_payload: Mutex<IslandPayload>,
    pub mic_test: Mutex<Option<Capture>>,
    pub download_cancel: Arc<AtomicBool>,
    pub downloading: Mutex<Option<String>>,
    partial_busy: Arc<AtomicBool>,
    /// Прерывание идущего финального распознавания (Esc во время обработки)
    final_abort: Mutex<Option<Arc<AtomicBool>>>,
}

impl Engine {
    pub fn new(app: AppHandle, dirs: Dirs) -> Result<Self, String> {
        let settings = Settings::load(&dirs.config);
        let history = History::open(&dirs.data)?;
        let reminders = Reminders::open(&dirs.data)?;
        history.apply_retention(settings.history_days);
        Ok(Self {
            app,
            dirs,
            settings: RwLock::new(settings),
            history,
            reminders,
            rem_island: Mutex::new(IslandReminders::default()),
            hit_rect: Mutex::new(None),
            local: LocalEngine::new(),
            session: Mutex::new(None),
            busy: AtomicBool::new(false),
            seq: AtomicU64::new(1),
            last_payload: Mutex::new(IslandPayload { phase: "hidden".into(), ..Default::default() }),
            mic_test: Mutex::new(None),
            download_cancel: Arc::new(AtomicBool::new(false)),
            downloading: Mutex::new(None),
            partial_busy: Arc::new(AtomicBool::new(false)),
            final_abort: Mutex::new(None),
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().clone()
    }

    pub fn is_recording(&self) -> bool {
        self.session.lock().is_some()
    }

    pub fn emit(&self, p: IslandPayload) {
        *self.last_payload.lock() = p.clone();
        let _ = self.app.emit("island", &p);
        self.sync_island_visibility();
    }

    /// Остров виден, если идёт диктовка, либо есть закреплённые маячки / «Пора!» / «Поставил»
    pub fn sync_island_visibility(&self) {
        let s = self.settings();
        let phase = self.last_payload.lock().phase.clone();
        let dictation_quiet = phase == "hidden" || (phase == "idle" && s.island.hide_idle);
        let rem = {
            let r = self.rem_island.lock();
            !r.pins.is_empty() || r.firing.is_some() || r.just_set.is_some()
        };
        if dictation_quiet && !rem {
            island::hide(&self.app);
        } else {
            island::show(&self.app, &s);
        }
    }

    fn idle_phase(&self) -> IslandPayload {
        let phase = if self.settings().island.hide_idle { "hidden" } else { "idle" };
        IslandPayload { phase: phase.into(), ..Default::default() }
    }

    fn flash(self: &Arc<Self>, phase: &str, message: &str, words: i64, hold: Duration) {
        self.emit(IslandPayload { phase: phase.into(), message: message.into(), words, ..Default::default() });
        let me = self.clone();
        let id = self.seq.load(Ordering::SeqCst);
        std::thread::spawn(move || {
            std::thread::sleep(hold);
            // Если за это время началась новая запись — не трогаем остров
            if me.seq.load(Ordering::SeqCst) == id && !me.is_recording() && !me.busy.load(Ordering::SeqCst) {
                let p = me.idle_phase();
                me.emit(p);
            }
        });
    }

    /// Прогреваем модель заранее, чтобы первая фраза не ждала загрузку с диска.
    pub fn warm_up(self: &Arc<Self>) {
        let s = self.settings();
        if s.engine != "local" || !models::is_installed(&self.dirs.models, &s.local_model) {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            if let Some(p) = models::path(&me.dirs.models, &s.local_model) {
                if let Err(e) = me.local.ensure_loaded(&s.local_model, &p, s.use_gpu) {
                    log::warn!("Прогрев модели не удался: {e}");
                }
            }
        });
    }

    pub fn on_hotkey(self: &Arc<Self>, e: hotkey::HotkeyEvent) {
        use hotkey::HotkeyEvent::*;
        let key_mode = self.settings().key_mode;
        match e {
            DictateDown => {
                let current = self.session.lock().as_ref().map(|s| s.mode);
                match current {
                    None if key_mode == "toggle" => self.start(Mode::Toggle, false),
                    None => self.start(Mode::Hold, false),
                    Some(Mode::Toggle) | Some(Mode::Locked) => self.stop(),
                    Some(Mode::Hold) => {}
                }
            }
            DictateUp => {
                if self.session.lock().as_ref().map(|s| s.mode == Mode::Hold).unwrap_or(false) {
                    self.stop();
                }
            }
            DictateAbort => {
                if self.session.lock().as_ref().map(|s| s.mode == Mode::Hold).unwrap_or(false) {
                    self.cancel(false);
                }
            }
            HandsFree => {
                let mut guard = self.session.lock();
                match guard.as_mut() {
                    Some(s) if s.mode == Mode::Locked => {
                        drop(guard);
                        self.stop();
                    }
                    Some(s) => {
                        s.mode = Mode::Locked;
                        drop(guard);
                    }
                    None => {
                        drop(guard);
                        self.start(Mode::Locked, false);
                    }
                }
            }
            Cancel => {
                // Esc сразу после «Маячок поставлен» — передумал
                let just = self.rem_island.lock().just_set.take();
                if let Some(r) = just {
                    let _ = self.reminders.delete(r.id);
                    hotkey::set_recording(self.is_recording());
                    self.refresh_reminders(true);
                    return;
                }
                let processing = match self.final_abort.lock().as_ref() {
                    Some(a) => {
                        a.store(true, Ordering::SeqCst);
                        true
                    }
                    None => false,
                };
                if processing && !self.is_recording() {
                    self.seq.fetch_add(1, Ordering::SeqCst);
                    let p = self.idle_phase();
                    self.emit(p);
                } else {
                    self.cancel(true);
                }
            }
            PasteLast => {
                if let Some(last) = self.history.last() {
                    let restore = self.settings().clipboard_mode == "restore";
                    std::thread::spawn(move || {
                        let _ = platform::paste_text(&last.text, restore);
                    });
                }
            }
            OpenApp => crate::show_main(&self.app),
            Captured(keys) => {
                let _ = self.app.emit("hotkey-captured", keys);
            }
        }
    }

    pub fn start(self: &Arc<Self>, mode: Mode, from_ui: bool) {
        if self.is_recording() || self.busy.load(Ordering::SeqCst) {
            return;
        }
        let s = self.settings();
        if s.is_paused() {
            self.flash("error", "Маячок на паузе", 0, Duration::from_millis(1600));
            return;
        }
        let target = if from_ui { ForegroundApp { is_self: true, name: "Маячок".into(), ..Default::default() } } else { platform::foreground_app() };
        if s.excluded_apps.iter().any(|a| a.eq_ignore_ascii_case(&target.exe) || a.eq_ignore_ascii_case(&target.name)) {
            self.flash("error", "Здесь диктовка выключена", 0, Duration::from_millis(1600));
            return;
        }
        if s.engine == "local" && !models::is_installed(&self.dirs.models, &s.local_model) {
            self.flash("error", "Модель не скачана — открой настройки", 0, Duration::from_millis(2600));
            return;
        }
        // Микрофон-тест в настройках держит устройство — отпускаем
        self.mic_test.lock().take();

        let capture = match Capture::start(s.mic_device.as_deref()) {
            Ok(c) => c,
            Err(e) => {
                self.flash("error", &e, 0, Duration::from_millis(2600));
                return;
            }
        };
        let id = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let partial = Arc::new(Mutex::new(String::new()));
        let abort = Arc::new(AtomicBool::new(false));
        *self.session.lock() = Some(Session {
            id,
            capture,
            started: Instant::now(),
            mode,
            target: target.clone(),
            from_ui,
            partial: partial.clone(),
            abort: abort.clone(),
        });
        hotkey::set_recording(true);
        if s.sounds {
            platform::play_sound(true);
        }
        tray::set_recording(&self.app, true);
        self.emit(IslandPayload { phase: "listening".into(), mode: Some(mode), app: target.name.clone(), ..Default::default() });
        self.warm_up();

        // Тикер: уровень громкости, таймер, живой предпросмотр
        let me = self.clone();
        std::thread::spawn(move || {
            // Живой предпросмотр гоняет модель каждые ~1,3 с. На видеокарте это бесплатно,
            // на процессоре с большой моделью он съедает все ядра и тормозит финальный проход
            let light_model = matches!(s.local_model.as_str(), "base" | "small");
            let live = s.engine == "local" && s.island.size == "text" && (me.local.backend().gpu || light_model);
            let mut last_partial = Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(50));
                let (level, elapsed, mode, snapshot) = {
                    let guard = me.session.lock();
                    let Some(sess) = guard.as_ref().filter(|x| x.id == id) else { break };
                    let elapsed = sess.started.elapsed();
                    let want_partial = live
                        && last_partial.elapsed() > Duration::from_millis(1300)
                        && sess.capture.seconds() > 1.0
                        && !me.partial_busy.load(Ordering::SeqCst);
                    let snap = if want_partial { Some(sess.capture.snapshot_16k()) } else { None };
                    (sess.capture.level(), elapsed, sess.mode, snap)
                };
                let _ = me.app.emit("level", level);
                if elapsed > MAX_RECORDING {
                    me.stop();
                    break;
                }
                let text = partial.lock().clone();
                let p = IslandPayload { phase: "listening".into(), mode: Some(mode), text, elapsed_ms: elapsed.as_millis() as u64, app: target.name.clone(), ..Default::default() };
                *me.last_payload.lock() = p.clone();
                let _ = me.app.emit("island", &p);

                if let Some(mut snap) = snapshot {
                    last_partial = Instant::now();
                    me.partial_busy.store(true, Ordering::SeqCst);
                    let (me2, partial2, abort2) = (me.clone(), partial.clone(), abort.clone());
                    std::thread::spawn(move || {
                        // Берём только последние 20 секунд — предпросмотру хватит
                        if snap.len() > 16_000 * 20 {
                            snap.drain(..snap.len() - 16_000 * 20);
                        }
                        let s = me2.settings();
                        if let Some(path) = models::path(&me2.dirs.models, &s.local_model) {
                            if me2.local.ensure_loaded(&s.local_model, &path, s.use_gpu).is_ok() {
                                if let Ok(t) = me2.local.transcribe(&snap, &s, true, abort2, Duration::from_secs(15)) {
                                    if me2.session.lock().as_ref().map(|x| x.id == id).unwrap_or(false) && !transcribe_is_noise(&t.text) {
                                        *partial2.lock() = t.text;
                                    }
                                }
                            }
                        }
                        me2.partial_busy.store(false, Ordering::SeqCst);
                    });
                }
            }
        });
    }

    pub fn cancel(self: &Arc<Self>, with_sound: bool) {
        let Some(sess) = self.session.lock().take() else { return };
        sess.abort.store(true, Ordering::SeqCst);
        drop(sess);
        hotkey::set_recording(false);
        tray::set_recording(&self.app, false);
        if with_sound && self.settings().sounds {
            platform::play_sound(false);
        }
        self.seq.fetch_add(1, Ordering::SeqCst);
        let p = self.idle_phase();
        self.emit(p);
    }

    pub fn stop(self: &Arc<Self>) {
        let Some(sess) = self.session.lock().take() else { return };
        // Предпросмотр больше не нужен — освобождаем модель под финальный проход
        sess.abort.store(true, Ordering::SeqCst);
        tray::set_recording(&self.app, false);
        let s = self.settings();
        if s.sounds {
            platform::play_sound(false);
        }
        let duration = sess.started.elapsed();
        let mut samples = sess.capture.stop();
        if duration < MIN_RECORDING {
            hotkey::set_recording(false);
            let p = self.idle_phase();
            self.emit(p);
            return;
        }
        if audio::rms(&samples) < 0.0015 {
            hotkey::set_recording(false);
            self.flash("error", "Тишина — ничего не услышал", 0, Duration::from_millis(1800));
            return;
        }
        // Esc во время обработки отменяет распознавание — хук продолжает его ловить
        let final_abort = Arc::new(AtomicBool::new(false));
        *self.final_abort.lock() = Some(final_abort.clone());
        self.busy.store(true, Ordering::SeqCst);
        self.emit(IslandPayload { phase: "processing".into(), message: "Распознаю…".into(), app: sess.target.name.clone(), ..Default::default() });

        let me = self.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            if s.noise_filter {
                audio::clean(&mut samples);
            }
            let result = if s.engine == "cloud" {
                transcribe::cloud(&samples, &s)
            } else {
                // Таймаут растёт с длиной записи, но «вечно» больше не бывает
                let timeout = Duration::from_secs_f32((samples.len() as f32 / 16_000.0 * 4.0).max(60.0));
                models::path(&me.dirs.models, &s.local_model)
                    .ok_or_else(|| "Неизвестная модель".to_string())
                    .and_then(|p| me.local.ensure_loaded(&s.local_model, &p, s.use_gpu))
                    .and_then(|_| me.local.transcribe(&samples, &s, false, final_abort.clone(), timeout))
            };
            let process_ms = t.elapsed().as_millis() as i64;
            me.busy.store(false, Ordering::SeqCst);
            *me.final_abort.lock() = None;
            hotkey::set_recording(false);
            if final_abort.load(Ordering::SeqCst) {
                // Отменил сам — остров уже спрятан в cancel()
                return;
            }
            let tr = match result {
                Ok(tr) => tr,
                Err(e) => {
                    log::error!("Распознавание: {e}");
                    me.flash("error", &e, 0, Duration::from_millis(3000));
                    return;
                }
            };
            if transcribe_is_noise(&tr.text) {
                me.flash("error", "Ничего не разобрал", 0, Duration::from_millis(1800));
                return;
            }
            // «напомни…», «маячок…», «срочно…» — это не текст, а маячок
            if s.reminders.enabled {
                let now_local = chrono::Local::now().naive_local();
                if let Some(parsed) = crate::remind_parse::parse(&tr.text, &s.reminders.triggers, now_local, s.reminders.default_hour) {
                    match me.reminders.create(&parsed, &tr.text, &sess.target.name) {
                        Ok(r) => me.announce_reminder(r),
                        Err(e) => me.flash("error", &format!("Маячок не сохранился: {e}"), 0, Duration::from_millis(2500)),
                    }
                    return;
                }
            }
            let text = crate::postprocess::process(&tr.text, &s);
            let words = count_words(&text);

            let paste_result = if sess.from_ui {
                platform::copy_text(&text)
            } else {
                platform::paste_text(&text, s.clipboard_mode == "restore")
            };
            if let Err(e) = &paste_result {
                log::error!("Вставка: {e}");
            }

            let audio_path = if s.keep_audio {
                let p = me.history.audio_dir.join(format!("{}.wav", uuid::Uuid::new_v4()));
                audio::write_wav(&p, &samples).ok().map(|_| p.to_string_lossy().to_string())
            } else {
                None
            };
            let model = if s.engine == "cloud" { s.cloud.model.clone() } else { s.local_model.clone() };
            match me.history.insert(NewEntry {
                text: text.clone(),
                raw_text: tr.text.clone(),
                app: sess.target.name.clone(),
                duration_ms: duration.as_millis() as i64,
                language: tr.language.clone(),
                model,
                engine: s.engine.clone(),
                process_ms,
                audio_path,
            }) {
                Ok(_) => {
                    let _ = me.app.emit("history-changed", ());
                }
                Err(e) => log::error!("История: {e}"),
            }
            me.history.apply_retention(s.history_days);
            let msg = match (&paste_result, sess.from_ui) {
                (Err(_), _) => "Скопировано — вставь сам".to_string(),
                (Ok(_), true) => "Скопировано".to_string(),
                (Ok(_), false) => "Вставлено".to_string(),
            };
            let _ = partial_cleanup(&sess.partial);
            me.flash("done", &msg, words, Duration::from_millis(1700));
            tray::refresh(&me.app);
        });
    }

    pub fn start_mic_test(&self, device: Option<String>) -> Result<String, String> {
        if self.is_recording() {
            return Err("Идёт запись".into());
        }
        let cap = Capture::start(device.as_deref())?;
        let name = cap.device_name.clone();
        *self.mic_test.lock() = Some(cap);
        Ok(name)
    }

    pub fn mic_test_level(&self) -> f32 {
        self.mic_test.lock().as_ref().map(|c| c.level()).unwrap_or(0.0)
    }

    pub fn stop_mic_test(&self) {
        self.mic_test.lock().take();
    }
}

fn transcribe_is_noise(text: &str) -> bool {
    crate::postprocess::is_hallucination(text)
}

fn partial_cleanup(p: &Arc<Mutex<String>>) -> String {
    std::mem::take(&mut *p.lock())
}

pub fn state(app: &AppHandle) -> Arc<Engine> {
    app.state::<Arc<Engine>>().inner().clone()
}

// ======================= Маячки =======================

const SET_HOLD: Duration = Duration::from_millis(3500);
const FIRE_HOLD: Duration = Duration::from_secs(120);

impl Engine {
    /// Показать «Маячок поставлен» на пару секунд; Esc в это время отменяет
    pub fn announce_reminder(self: &Arc<Self>, r: rem::Reminder) {
        let id = r.id;
        self.rem_island.lock().just_set = Some(r);
        hotkey::set_recording(true);
        // Диктовочная часть острова гаснет — дальше показывает маячок
        let p = self.idle_phase();
        *self.last_payload.lock() = p.clone();
        let _ = self.app.emit("island", &p);
        self.refresh_reminders(true);
        let me = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(SET_HOLD);
            let mut st = me.rem_island.lock();
            if st.just_set.as_ref().map(|x| x.id == id).unwrap_or(false) {
                st.just_set = None;
                drop(st);
                hotkey::set_recording(me.is_recording());
                me.refresh_reminders(true);
            }
        });
    }

    /// Пересчитать закреплённые и, если что-то поменялось (или `force`), сообщить окнам
    pub fn refresh_reminders(&self, force: bool) {
        let s = self.settings();
        let active = self.reminders.list(false).unwrap_or_default();
        let pins = rem::pins(&active, chrono::Utc::now(), s.reminders.pin_before_min, s.reminders.urgent_pins);
        let payload = {
            let mut st = self.rem_island.lock();
            // «Пора!» для удалённого/выполненного — убрать
            if let Some(f) = &st.firing {
                if !active.iter().any(|a| a.id == f.id && a.fired) {
                    st.firing = None;
                }
            }
            let key = |v: &Vec<rem::Reminder>| v.iter().map(|r| (r.id, r.due_at.clone(), r.fired, r.text.clone(), r.urgent)).collect::<Vec<_>>();
            let changed = key(&st.pins) != key(&pins);
            if !changed && !force {
                return;
            }
            st.pins = pins;
            st.clone()
        };
        let _ = self.app.emit("island-reminders", &payload);
        let _ = self.app.emit("reminders-changed", ());
        self.sync_island_visibility();
        tray::refresh(&self.app);
    }

    fn fire(self: &Arc<Self>, r: rem::Reminder) {
        let s = self.settings();
        self.reminders.mark_fired(r.id);
        let mut fired = r.clone();
        fired.fired = true;
        self.rem_island.lock().firing = Some(fired);
        log::info!("Маячок #{}: «{}»", r.id, r.text);
        if s.reminders.sound {
            platform::play_sound(true);
        }
        if s.reminders.windows_toast && r.toast {
            let when = r.due().with_timezone(&chrono::Local).format("%H:%M").to_string();
            crate::toast::show_reminder(&self.app, r.id, &r.text, &format!("Сейчас · {when}"), true);
        }
        self.refresh_reminders(true);
        let (me, id) = (self.clone(), r.id);
        std::thread::spawn(move || {
            std::thread::sleep(FIRE_HOLD);
            let mut st = me.rem_island.lock();
            if st.firing.as_ref().map(|x| x.id == id).unwrap_or(false) {
                // Никто не нажал — сворачиваем в «просрочено» на острове
                st.firing = None;
                drop(st);
                me.refresh_reminders(true);
            }
        });
    }

    /// Раз в секунду: сработавшие, предупреждения заранее, закреплённые
    pub fn tick_reminders(self: &Arc<Self>) {
        let s = self.settings();
        let now = chrono::Utc::now();
        let active = match self.reminders.list(false) {
            Ok(a) => a,
            Err(_) => return,
        };
        for r in active {
            let due = r.due();
            let lead = if r.lead_min >= 0 { r.lead_min } else { s.reminders.lead_min as i64 };
            if !r.fired && now >= due {
                self.fire(r);
            } else if !r.fired && !r.pre_notified && lead > 0 && now >= due - chrono::Duration::minutes(lead) {
                self.reminders.mark_pre(r.id);
                if s.reminders.windows_toast && r.toast {
                    let left = ((due - now).num_seconds() as f64 / 60.0).ceil() as i64;
                    crate::toast::show_reminder(&self.app, r.id, &r.text, &format!("Через {left} мин"), false);
                }
            }
        }
        self.refresh_reminders(false);
    }

    /// Действие с кнопки острова, уведомления или из приложения
    pub fn reminder_action(self: &Arc<Self>, id: i64, action: &str) -> Result<(), String> {
        let s = self.settings();
        match action {
            "done" => self.reminders.done(id)?,
            "snooze" => self.reminders.snooze(id, s.reminders.snooze_min as i64)?,
            "hour" => self.reminders.snooze(id, 60)?,
            "tomorrow" => self.reminders.snooze_tomorrow(id, s.reminders.default_hour)?,
            "undone" => self.reminders.undone(id)?,
            "delete" => self.reminders.delete(id)?,
            _ => return Err("Неизвестное действие".into()),
        }
        {
            let mut st = self.rem_island.lock();
            if st.firing.as_ref().map(|x| x.id == id).unwrap_or(false) {
                st.firing = None;
            }
            if st.just_set.as_ref().map(|x| x.id == id).unwrap_or(false) {
                st.just_set = None;
            }
        }
        self.refresh_reminders(true);
        Ok(())
    }

    pub fn start_reminder_loop(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::Builder::new()
            .name("reminders".into())
            .spawn(move || loop {
                me.tick_reminders();
                std::thread::sleep(Duration::from_secs(1));
            })
            .expect("reminders thread");
    }

    /// Остров не мешает кликам, пока курсор не над ним: окно прозрачное и большое,
    /// поэтому включаем мышь только когда курсор внутри прямоугольника острова.
    pub fn start_hit_test_loop(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::Builder::new()
            .name("island-hit".into())
            .spawn(move || {
                let mut interactive = false;
                loop {
                    std::thread::sleep(Duration::from_millis(50));
                    let rect = *me.hit_rect.lock();
                    let Some(w) = me.app.get_webview_window(island::LABEL) else { continue };
                    let want = match rect {
                        Some([x, y, rw, rh]) if w.is_visible().unwrap_or(false) => {
                            match (me.app.cursor_position(), w.outer_position(), w.scale_factor()) {
                                (Ok(c), Ok(pos), Ok(sf)) => {
                                    let (lx, ly) = ((c.x - pos.x as f64) / sf, (c.y - pos.y as f64) / sf);
                                    lx >= x && lx <= x + rw && ly >= y && ly <= y + rh
                                }
                                _ => false,
                            }
                        }
                        _ => false,
                    };
                    if want != interactive {
                        interactive = want;
                        let _ = w.set_ignore_cursor_events(!want);
                        let _ = me.app.emit("island-hover", want);
                    }
                }
            })
            .expect("hit-test thread");
    }
}
