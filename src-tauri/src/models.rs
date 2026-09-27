//! Каталог моделей whisper.cpp и их загрузка.

use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct ModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    pub desc: &'static str,
    pub recommended: bool,
}

pub const MODELS: &[ModelDef] = &[
    ModelDef {
        id: "large-v3-turbo-q5_0",
        name: "Whisper large-v3-turbo (q5)",
        file: "ggml-large-v3-turbo-q5_0.bin",
        size_mb: 574,
        desc: "Почти как полная turbo, но втрое легче. Лучший баланс для большинства ПК.",
        recommended: true,
    },
    ModelDef {
        id: "large-v3-turbo",
        name: "Whisper large-v3-turbo",
        file: "ggml-large-v3-turbo.bin",
        size_mb: 1624,
        desc: "Максимальная точность. Лучше с мощным процессором или видеокартой.",
        recommended: false,
    },
    ModelDef {
        id: "small",
        name: "Whisper small",
        file: "ggml-small.bin",
        size_mb: 488,
        desc: "Быстрее на слабых ноутбуках, ошибается чаще.",
        recommended: false,
    },
    ModelDef {
        id: "base",
        name: "Whisper base",
        file: "ggml-base.bin",
        size_mb: 148,
        desc: "Самая лёгкая. Подойдёт, чтобы просто попробовать.",
        recommended: false,
    },
];

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub desc: String,
    pub recommended: bool,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub id: String,
    pub downloaded: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

pub fn def(id: &str) -> Option<&'static ModelDef> {
    MODELS.iter().find(|m| m.id == id)
}

pub fn path(dir: &Path, id: &str) -> Option<PathBuf> {
    def(id).map(|m| dir.join(m.file))
}

pub fn is_installed(dir: &Path, id: &str) -> bool {
    path(dir, id).map(|p| p.exists()).unwrap_or(false)
}

pub fn list(dir: &Path) -> Vec<ModelInfo> {
    MODELS
        .iter()
        .map(|m| ModelInfo {
            id: m.id.into(),
            name: m.name.into(),
            size_mb: m.size_mb,
            desc: m.desc.into(),
            recommended: m.recommended,
            installed: dir.join(m.file).exists(),
        })
        .collect()
}

pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    let p = path(dir, id).ok_or("Нет такой модели")?;
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Качает модель, вызывая `on_progress` не чаще ~10 раз в секунду.
pub fn download(
    dir: &Path,
    id: &str,
    cancel: Arc<AtomicBool>,
    mut on_progress: impl FnMut(Progress),
) -> Result<(), String> {
    let m = def(id).ok_or("Нет такой модели")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let final_path = dir.join(m.file);
    let part = dir.join(format!("{}.part", m.file));

    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .connect_timeout(std::time::Duration::from_secs(20))
        .user_agent(concat!("Mayachok/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client
        .get(format!("{BASE_URL}{}", m.file))
        .send()
        .map_err(|e| format!("Нет соединения: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Сервер ответил {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(m.size_mb as u64 * 1024 * 1024);
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut downloaded = 0u64;
    let mut last = std::time::Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err("Загрузка отменена".into());
        }
        let n = resp.read(&mut buf).map_err(|e| format!("Обрыв загрузки: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        downloaded += n as u64;
        if last.elapsed().as_millis() > 100 {
            last = std::time::Instant::now();
            on_progress(Progress { id: id.into(), downloaded, total, done: false, error: None });
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    if downloaded < total / 2 {
        let _ = std::fs::remove_file(&part);
        return Err("Файл модели скачался не полностью".into());
    }
    std::fs::rename(&part, &final_path).map_err(|e| e.to_string())?;
    on_progress(Progress { id: id.into(), downloaded, total, done: true, error: None });
    Ok(())
}
