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

#[cfg(feature = "local-whisper")]
pub mod local {
    use super::*;
    use parking_lot::Mutex;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

    pub struct Loaded {
        pub model_id: String,
        _ctx: WhisperContext,
        state: WhisperState,
    }

    /// Загруженная модель; держим одну, чтобы не читать гигабайт с диска на каждую фразу.
    pub struct LocalEngine {
        inner: Mutex<Option<Loaded>>,
    }

    impl LocalEngine {
        pub fn new() -> Self {
            whisper_rs::install_logging_hooks();
            Self { inner: Mutex::new(None) }
        }

        pub fn loaded_model(&self) -> Option<String> {
            self.inner.lock().as_ref().map(|l| l.model_id.clone())
        }

        pub fn unload(&self) {
            *self.inner.lock() = None;
        }

        pub fn ensure_loaded(&self, model_id: &str, path: &Path) -> Result<(), String> {
            let mut guard = self.inner.lock();
            if guard.as_ref().map(|l| l.model_id == model_id).unwrap_or(false) {
                return Ok(());
            }
            *guard = None;
            if !path.exists() {
                return Err("Модель не загружена — скачай её в настройках".into());
            }
            let mut params = WhisperContextParameters::default();
            params.use_gpu(cfg!(any(feature = "cuda", feature = "vulkan")));
            params.flash_attn(cfg!(any(feature = "cuda", feature = "vulkan")));
            let t = std::time::Instant::now();
            let ctx = WhisperContext::new_with_params(&path.to_string_lossy(), params)
                .map_err(|e| format!("Не удалось загрузить модель: {e}"))?;
            let state = ctx.create_state().map_err(|e| format!("Не удалось подготовить модель: {e}"))?;
            log::info!("Модель {model_id} загружена за {:?}", t.elapsed());
            *guard = Some(Loaded { model_id: model_id.into(), _ctx: ctx, state });
            Ok(())
        }

        /// `quick` — облегчённый проход для живого предпросмотра.
        pub fn transcribe(&self, samples: &[f32], s: &Settings, quick: bool) -> Result<Transcript, String> {
            let mut guard = self.inner.lock();
            let loaded = guard.as_mut().ok_or("Модель не загружена")?;
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
            p.set_n_threads(threads as i32);
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
                p.set_temperature_inc(0.0);
            }
            let lang = if s.auto_detect { None } else { Some(s.language.as_str()) };
            p.set_language(lang.or(Some("auto")));
            let prompt = build_prompt(s);
            if !prompt.is_empty() {
                p.set_initial_prompt(&prompt);
            }
            // whisper не любит отрезки короче секунды — добиваем тишиной
            let mut audio = samples.to_vec();
            if audio.len() < 16_000 + 1600 {
                audio.resize(16_000 + 1600, 0.0);
            }
            loaded.state.full(p, &audio).map_err(|e| format!("Ошибка распознавания: {e}"))?;
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
    pub struct LocalEngine;
    impl LocalEngine {
        pub fn new() -> Self {
            Self
        }
        pub fn loaded_model(&self) -> Option<String> {
            None
        }
        pub fn unload(&self) {}
        pub fn ensure_loaded(&self, _: &str, _: &Path) -> Result<(), String> {
            Err("Эта сборка без локального распознавания — выбери облако".into())
        }
        pub fn transcribe(&self, _: &[f32], _: &Settings, _: bool) -> Result<Transcript, String> {
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
