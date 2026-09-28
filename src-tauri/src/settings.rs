use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Hotkeys {
    pub dictate: Vec<String>,
    pub hands_free: Vec<String>,
    pub cancel: Vec<String>,
    pub paste_last: Vec<String>,
    pub open_app: Vec<String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            dictate: vec!["Ctrl".into(), "Win".into()],
            hands_free: vec!["Ctrl".into(), "Win".into(), "Space".into()],
            cancel: vec!["Esc".into()],
            paste_last: vec!["Alt".into(), "Shift".into(), "V".into()],
            open_app: vec!["Ctrl".into(), "Alt".into(), "M".into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Island {
    /// top | bottom | cursor
    pub position: String,
    /// compact | normal | text
    pub size: String,
    pub accent: String,
    pub hide_idle: bool,
}

impl Default for Island {
    fn default() -> Self {
        Self {
            position: "top".into(),
            size: "text".into(),
            accent: "#ff453a".into(),
            hide_idle: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Cloud {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl Default for Cloud {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model: "whisper-1".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub from: String,
    pub to: String,
    /// command | replace | snippet
    pub kind: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Dictionary {
    pub words: Vec<String>,
    pub rules: Vec<Rule>,
}

impl Default for Dictionary {
    fn default() -> Self {
        let rule = |id: &str, from: &str, to: &str, kind: &str| Rule {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            kind: kind.into(),
            enabled: true,
        };
        Self {
            words: vec!["Маячок".into()],
            rules: vec![
                rule("cmd-newline", "новая строка", "\n", "command"),
                rule("cmd-paragraph", "новый абзац", "\n\n", "command"),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub onboarded: bool,
    // Общие
    pub autostart: bool,
    pub close_to_tray: bool,
    pub ui_language: String,
    /// dark | light | system
    pub theme: String,
    pub live_type: bool,
    /// 0..=1
    pub live_amplitude: f32,
    pub live_pause_unfocused: bool,
    pub sounds: bool,
    // Звук и распознавание
    pub mic_device: Option<String>,
    pub noise_filter: bool,
    /// local | cloud
    pub engine: String,
    pub local_model: String,
    /// Считать на видеокарте, если сборка и драйвер это умеют
    pub use_gpu: bool,
    pub cloud: Cloud,
    /// ru | en | uk | ...
    pub language: String,
    pub auto_detect: bool,
    pub auto_punctuation: bool,
    pub remove_fillers: bool,
    pub voice_commands: bool,
    // Клавиши
    pub hotkeys: Hotkeys,
    /// hold | toggle
    pub key_mode: String,
    /// restore | keep
    pub clipboard_mode: String,
    // Остров
    pub island: Island,
    // Приватность
    pub history_days: u32,
    pub keep_audio: bool,
    pub excluded_apps: Vec<String>,
    // Словарь
    pub dictionary: Dictionary,
    // О программе
    pub auto_update: bool,
    /// stable | beta
    pub update_channel: String,
    /// unix ms, пауза диктовки
    pub paused_until: Option<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            autostart: true,
            close_to_tray: true,
            ui_language: "ru".into(),
            theme: "dark".into(),
            live_type: true,
            live_amplitude: 0.6,
            live_pause_unfocused: true,
            sounds: false,
            mic_device: None,
            noise_filter: true,
            engine: "local".into(),
            local_model: "large-v3-turbo-q5_0".into(),
            use_gpu: true,
            cloud: Cloud::default(),
            language: "ru".into(),
            auto_detect: true,
            auto_punctuation: true,
            remove_fillers: true,
            voice_commands: true,
            hotkeys: Hotkeys::default(),
            key_mode: "hold".into(),
            clipboard_mode: "restore".into(),
            island: Island::default(),
            history_days: 30,
            keep_audio: true,
            excluded_apps: vec!["KeePassXC".into(), "1Password".into(), "Bitwarden".into()],
            dictionary: Dictionary::default(),
            auto_update: true,
            update_channel: "stable".into(),
            paused_until: None,
        }
    }
}

impl Settings {
    pub fn path(dir: &PathBuf) -> PathBuf {
        dir.join("settings.json")
    }

    pub fn load(dir: &PathBuf) -> Self {
        let path = Self::path(dir);
        match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                log::warn!("settings.json повреждён ({e}), беру значения по умолчанию");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, dir: &PathBuf) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let tmp = dir.join("settings.json.tmp");
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&tmp, data).map_err(|e| e.to_string())?;
        fs::rename(&tmp, Self::path(dir)).map_err(|e| e.to_string())
    }

    pub fn is_paused(&self) -> bool {
        self.paused_until
            .map(|t| t > chrono::Utc::now().timestamp_millis())
            .unwrap_or(false)
    }
}
