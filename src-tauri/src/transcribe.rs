//! Распознавание речи: локально (whisper.cpp) или через OpenAI-совместимый API.

use crate::settings::Settings;
use std::path::Path;

pub struct Transcript {
    pub text: String,
    pub language: String,
}

/// Подсказка модели: свои слова из словаря + пример пунктуации.
pub fn build_prompt(s: &Settings) -> String {
    let mut prompt = String::new();
    if s.auto_punctuation {
        prompt.push_str(match s.language.as_str() {
            "en" => "Hello. This is a dictation with proper punctuation. ",
            _ => "Привет. Это диктовка, с запятыми и точками. ",
        });
    }
    let words: Vec<&str> = s.dictionary.words.iter().map(|w| w.trim()).filter(|w| !w.is_empty()).collect();
    if !words.is_empty() {
        prompt.push_str(&words.join(", "));
        prompt.push('.');
    }
    prompt
}

/// Начало сообщения об ошибке самого whisper.cpp — по нему движок решает, пробовать ли запасной режим
pub const WHISPER_FAIL: &str = "Ошибка распознавания";

/// Как прошло распознавание — для лога и диагностики
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backend {
    /// «Видеокарта: NVIDIA GeForce RTX 3060» или «Процессор»
    pub device: String,
    pub gpu: bool,
    /// Флаги процессора из whisper.cpp: AVX2, FMA…
    pub cpu_features: String,
    pub threads: usize,
}

pub fn threads() -> usize {
    // Одно ядро оставляем системе: иначе на CPU подтормаживает всё, включая клавиатуру
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).clamp(2, 8)
}

/// Сколько кадров энкодера нужно под длину звука. Whisper всегда считает окно на 30 секунд,
/// и для короткой фразы это 90% впустую — поэтому ограничиваем контекст (как `-ac` в whisper.cpp).
pub fn audio_ctx(samples: usize, quick: bool) -> i32 {
    let secs = samples as f32 / 16_000.0;
    if secs >= 28.0 {
        return 0; // полное окно
    }
    let frames = (secs * 50.0).ceil() as i32 + 64; // 50 кадров на секунду + запас
    let min = if quick { 256 } else { 384 };
    ((frames + 63) / 64 * 64).clamp(min, 1500)
}

#[cfg(feature = "local-whisper")]
pub mod local {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError, WhisperState};

    pub struct Loaded {
        pub model_id: String,
        gpu: bool,
        flash: bool,
        _ctx: WhisperContext,
        state: WhisperState,
    }

    /// Загруженная модель; держим одну, чтобы не читать гигабайт с диска на каждую фразу.
    pub struct LocalEngine {
        /// Держится всё время распознавания — не трогать из главного потока!
        inner: Mutex<Option<Loaded>>,
        /// Лёгкие копии для интерфейса: их можно читать, пока модель занята
        backend: Mutex<Backend>,
        loaded_id: Mutex<Option<String>>,
        /// Запасные режимы, если whisper.cpp падает на быстрых настройках:
        /// 0 — flash attention + урезанный audio_ctx; 1 — полное окно 30 с; 2 — процессор без flash attention
        safe: AtomicU8,
    }

    pub const GPU_BUILD: bool = cfg!(any(feature = "cuda", feature = "vulkan"));

    /// Видеокарты, которые видит ggml (пусто в сборке без GPU или без драйвера)
    pub fn gpu_devices() -> Vec<String> {
        let mut out = vec![];
        unsafe {
            for i in 0..whisper_rs_sys::ggml_backend_dev_count() {
                let dev = whisper_rs_sys::ggml_backend_dev_get(i);
                if whisper_rs_sys::ggml_backend_dev_type(dev) == whisper_rs_sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU {
                    let d = whisper_rs_sys::ggml_backend_dev_description(dev);
                    if !d.is_null() {
                        out.push(std::ffi::CStr::from_ptr(d).to_string_lossy().trim().to_string());
                    }
                }
            }
        }
        out
    }

    pub fn cpu_features() -> String {
        let info = whisper_rs::print_system_info();
        // «AVX = 1 | AVX2 = 1 | …» → «AVX AVX2 …»
        info.split('|')
            .filter_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                (v.trim() == "1").then(|| k.trim().to_string())
            })
            .filter(|k| !k.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    struct AbortCtx {
        abort: Arc<AtomicBool>,
        deadline: Instant,
        timed_out: AtomicBool,
    }

    unsafe extern "C" fn abort_cb(data: *mut std::ffi::c_void) -> bool {
        if data.is_null() {
            return false;
        }
        let c = &*(data as *const AbortCtx);
        if Instant::now() > c.deadline {
            c.timed_out.store(true, Ordering::SeqCst);
            return true;
        }
        c.abort.load(Ordering::SeqCst)
    }

    impl LocalEngine {
        pub fn new() -> Self {
            whisper_rs::install_logging_hooks();
            let backend = Backend { device: "Процессор".into(), gpu: false, cpu_features: cpu_features(), threads: threads() };
            log::info!("whisper.cpp: {}", whisper_rs::print_system_info());
            Self { inner: Mutex::new(None), backend: Mutex::new(backend), loaded_id: Mutex::new(None), safe: AtomicU8::new(0) }
        }

        /// Перейти на более надёжный (и медленный) режим. `false` — дальше некуда.
        pub fn escalate(&self) -> bool {
            let cur = self.safe.load(Ordering::SeqCst);
            if cur >= 2 {
                return false;
            }
            self.safe.store(cur + 1, Ordering::SeqCst);
            log::warn!(
                "whisper: перехожу на запасной режим {} ({})",
                cur + 1,
                if cur + 1 == 1 { "полное окно 30 с" } else { "процессор без flash attention" }
            );
            true
        }

        pub fn loaded_model(&self) -> Option<String> {
            self.loaded_id.lock().clone()
        }

        pub fn backend(&self) -> Backend {
            self.backend.lock().clone()
        }

        /// Может ждать окончания распознавания — вызывать только из фонового потока
        pub fn unload(&self) {
            *self.inner.lock() = None;
            *self.loaded_id.lock() = None;
        }

        pub fn ensure_loaded(&self, model_id: &str, path: &Path, want_gpu: bool) -> Result<(), String> {
            let mut guard = self.inner.lock();
            let safe = self.safe.load(Ordering::SeqCst);
            let gpu = want_gpu && GPU_BUILD && safe < 2 && !gpu_devices().is_empty();
            let flash = safe < 2;
            if guard.as_ref().map(|l| l.model_id == model_id && l.gpu == gpu && l.flash == flash).unwrap_or(false) {
                return Ok(());
            }
            *guard = None;
            *self.loaded_id.lock() = None;
            if !path.exists() {
                return Err("Модель не загружена — скачай её в настройках".into());
            }
            let mut params = WhisperContextParameters::default();
            params.use_gpu(gpu);
            // Flash attention ускоряет и видеокарту, и процессор (кроме запасного режима)
            params.flash_attn(flash);
            let t = Instant::now();
            let ctx = WhisperContext::new_with_params(&path.to_string_lossy(), params)
                .map_err(|e| format!("Не удалось загрузить модель: {e}"))?;
            let state = ctx.create_state().map_err(|e| format!("Не удалось подготовить модель: {e}"))?;
            let device = if gpu {
                format!("Видеокарта: {}", gpu_devices().first().cloned().unwrap_or_default())
            } else {
                "Процессор".to_string()
            };
            log::info!("Модель {model_id} загружена за {:?} · {device}", t.elapsed());
            {
                let mut b = self.backend.lock();
                b.device = device;
                b.gpu = gpu;
            }
            *guard = Some(Loaded { model_id: model_id.into(), gpu, flash, _ctx: ctx, state });
            *self.loaded_id.lock() = Some(model_id.into());
            Ok(())
        }

        /// `quick` — облегчённый проход для живого предпросмотра.
        /// `abort` и `timeout` прерывают whisper посреди работы, а не ждут, пока он досчитает.
        pub fn transcribe(
            &self,
            samples: &[f32],
            s: &Settings,
            quick: bool,
            abort: Arc<AtomicBool>,
            timeout: Duration,
        ) -> Result<Transcript, String> {
            let mut guard = self.inner.lock();
            let loaded = guard.as_mut().ok_or("Модель не загружена")?;
            if abort.load(Ordering::SeqCst) {
                return Err("Прервано".into());
            }
            // whisper не любит отрезки короче секунды — добиваем тишиной
            let mut audio = samples.to_vec();
            if audio.len() < 16_000 + 1600 {
                audio.resize(16_000 + 1600, 0.0);
            }

            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            p.set_n_threads(threads() as i32);
            p.set_translate(false);
            p.set_no_context(true);
            p.set_no_timestamps(true);
            p.set_single_segment(quick);
            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p.set_suppress_blank(true);
            p.set_temperature(0.0);
            if quick {
                // Без повторных попыток с «температурой» — предпросмотру скорость важнее
                p.set_temperature_inc(0.0);
            }
            let ac = if self.safe.load(Ordering::SeqCst) == 0 { audio_ctx(audio.len(), quick) } else { 0 };
            p.set_audio_ctx(ac);
            let lang = if s.auto_detect { None } else { Some(s.language.as_str()) };
            p.set_language(lang.or(Some("auto")));
            let prompt = build_prompt(s);
            if !prompt.is_empty() {
                p.set_initial_prompt(&prompt);
            }
            // Свой колбэк прерывания вместо set_abort_callback_safe: в whisper-rs 0.14 тот
            // читает замыкание не по тому указателю (Box<Box<dyn FnMut>> как F), колбэк
            // возвращает мусор, энкодер «прерывается» и whisper_full отдаёт -6.
            let actx = Box::new(AbortCtx { abort: abort.clone(), deadline: Instant::now() + timeout, timed_out: AtomicBool::new(false) });
            // SAFETY: actx живёт до конца функции, то есть дольше вызова full()
            unsafe {
                p.set_abort_callback(Some(abort_cb));
                p.set_abort_callback_user_data(&*actx as *const AbortCtx as *mut std::ffi::c_void);
            }

            let t = Instant::now();
            let res = loaded.state.full(p, &audio);
            if actx.timed_out.load(Ordering::SeqCst) {
                return Err("Слишком долго — возьми модель полегче или включи видеокарту".into());
            }
            if abort.load(Ordering::SeqCst) {
                return Err("Прервано".into());
            }
            res.map_err(|e| match e {
                WhisperError::GenericError(code) => {
                    log::error!("whisper_full вернул {code} (режим {}, audio_ctx {ac}, GPU {})", self.safe.load(Ordering::SeqCst), loaded.gpu);
                    format!("{WHISPER_FAIL} (код {code})")
                }
                other => format!("{WHISPER_FAIL}: {other}"),
            })?;
            if !quick {
                log::info!(
                    "Распознано {:.1} с звука за {:?} ({}, потоков {}, audio_ctx {})",
                    samples.len() as f32 / 16_000.0,
                    t.elapsed(),
                    if loaded.gpu { "GPU" } else { "CPU" },
                    threads(),
                    ac
                );
            }
            let n = loaded.state.full_n_segments().map_err(|e| e.to_string())?;
            let mut text = String::new();
            for i in 0..n {
                if let Ok(seg) = loaded.state.full_get_segment_text_lossy(i) {
                    text.push_str(&seg);
                }
            }
            let language = loaded
                .state
                .full_lang_id_from_state()
                .ok()
                .and_then(whisper_rs::get_lang_str)
                .unwrap_or(&s.language)
                .to_string();
            Ok(Transcript { text: text.trim().to_string(), language })
        }
    }
}

#[cfg(not(feature = "local-whisper"))]
pub mod local {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;
    pub const GPU_BUILD: bool = false;
    pub struct LocalEngine;
    pub fn gpu_devices() -> Vec<String> {
        vec![]
    }
    impl LocalEngine {
        pub fn new() -> Self {
            Self
        }
        pub fn loaded_model(&self) -> Option<String> {
            None
        }
        pub fn backend(&self) -> Backend {
            Backend { device: "Облако".into(), ..Default::default() }
        }
        pub fn unload(&self) {}
        pub fn escalate(&self) -> bool {
            false
        }
        pub fn ensure_loaded(&self, _: &str, _: &Path, _: bool) -> Result<(), String> {
            Err("Эта сборка без локального распознавания — выбери облако".into())
        }
        pub fn transcribe(&self, _: &[f32], _: &Settings, _: bool, _: Arc<AtomicBool>, _: Duration) -> Result<Transcript, String> {
            Err("Эта сборка без локального распознавания".into())
        }
    }
}

/// OpenAI-совместимый `/audio/transcriptions` (OpenAI, Groq и т.п.).
pub fn cloud(samples: &[f32], s: &Settings) -> Result<Transcript, String> {
    if s.cloud.api_key.trim().is_empty() {
        return Err("Нет API-ключа — добавь его в настройках".into());
    }
    let wav = crate::audio::wav_bytes(samples);
    let part = reqwest::blocking::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let mut form = reqwest::blocking::multipart::Form::new()
        .text("model", s.cloud.model.clone())
        .text("response_format", "json")
        .part("file", part);
    if !s.auto_detect {
        form = form.text("language", s.language.clone());
    }
    let prompt = build_prompt(s);
    if !prompt.is_empty() {
        form = form.text("prompt", prompt);
    }
    let url = format!("{}/audio/transcriptions", s.cloud.base_url.trim_end_matches('/'));
    let resp = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?
        .post(url)
        .bearer_auth(s.cloud.api_key.trim())
        .multipart(form)
        .send()
        .map_err(|e| format!("Нет соединения: {e}"))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().map_err(|e| format!("Странный ответ сервера: {e}"))?;
    if !status.is_success() {
        let msg = body["error"]["message"].as_str().unwrap_or("неизвестная ошибка");
        return Err(format!("Облако: {msg}"));
    }
    Ok(Transcript {
        text: body["text"].as_str().unwrap_or_default().trim().to_string(),
        language: body["language"].as_str().unwrap_or(&s.language).to_string(),
    })
}
