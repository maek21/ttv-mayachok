//! Чистка текста после распознавания: паразиты, голосовые команды, замены из словаря.

use crate::settings::Settings;
use regex::{Regex, RegexBuilder};
use std::sync::OnceLock;

/// «ну,» / «типа,» / «короче,» (только с запятой) и междометия «э-э», «ммм».
fn simple_fillers() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        RegexBuilder::new(r"(^|[\s,.!?])(?:э+(?:-э+)*|эм+|мм+|хм+|ну|типа|короче|как бы)\s*,(\s|$)|(^|[\s,.!?])(?:э+(?:-э+)*|эм+|мм+|хм+)[.,]?(\s|$)")
            .case_insensitive(true)
            .build()
            .expect("filler regex")
    })
}

fn remove_fillers(text: &str) -> String {
    let mut out = text.to_string();
    // Два прохода — соседние паразиты делят пробел между собой
    for _ in 0..2 {
        out = simple_fillers()
            .replace_all(&out, |c: &regex::Captures| {
                let lead = c.get(1).or_else(|| c.get(3)).map(|m| m.as_str()).unwrap_or("");
                let tail = c.get(2).or_else(|| c.get(4)).map(|m| m.as_str()).unwrap_or("");
                format!("{lead}{tail}")
            })
            .into_owned();
    }
    out
}

fn phrase_regex(phrase: &str) -> Option<Regex> {
    let phrase = phrase.trim();
    if phrase.is_empty() {
        return None;
    }
    let words: Vec<String> = phrase.split_whitespace().map(regex::escape).collect();
    // Слова фразы через любые пробелы/запятые; вокруг — не буквы
    let body = words.join(r"[\s,]+");
    RegexBuilder::new(&format!(r"(^|[^\p{{L}}\p{{N}}])({body})([^\p{{L}}\p{{N}}]|$)"))
        .case_insensitive(true)
        .build()
        .ok()
}

fn apply_command(text: &str, phrase: &str, to: &str) -> String {
    let Some(re) = phrase_regex(phrase) else { return text.to_string() };
    // Команда съедает окружающую пунктуацию: «привет. Новая строка. Как дела» → «привет.\nКак дела»
    let mut out = text.to_string();
    while let Some(c) = re.captures(&out) {
        let m = c.get(2).unwrap();
        let mut start = m.start();
        let mut end = m.end();
        let bytes = out.as_bytes();
        while start > 0 && matches!(bytes[start - 1], b' ' | b',') {
            start -= 1;
        }
        while end < bytes.len() && matches!(bytes[end], b' ' | b',' | b'.' | b'!' | b'?' | b':' | b';') {
            end += 1;
        }
        let rest = out[end..].to_string();
        let mut rest_chars = rest.chars();
        let rest_cap = match rest_chars.next() {
            Some(f) if to.contains('\n') => f.to_uppercase().collect::<String>() + rest_chars.as_str(),
            Some(f) => f.to_string() + rest_chars.as_str(),
            None => String::new(),
        };
        let before = out[..start].to_string();
        let joined = if to.contains('\n') || before.is_empty() || rest_cap.is_empty() {
            format!("{before}{to}{rest_cap}")
        } else {
            format!("{before} {to} {rest_cap}")
        };
        if joined == out {
            break;
        }
        out = joined;
    }
    out
}

fn apply_replace(text: &str, from: &str, to: &str) -> String {
    let Some(re) = phrase_regex(from) else { return text.to_string() };
    re.replace_all(text, |c: &regex::Captures| format!("{}{}{}", &c[1], to, &c[3]))
        .into_owned()
}

fn strip_punctuation(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '.' | ',' | '!' | '?' | ';' | ':')).collect()
}

fn tidy(text: &str) -> String {
    static SPACES: OnceLock<Regex> = OnceLock::new();
    static BEFORE_PUNCT: OnceLock<Regex> = OnceLock::new();
    let spaces = SPACES.get_or_init(|| Regex::new(r"[ \t]{2,}").unwrap());
    let before = BEFORE_PUNCT.get_or_init(|| Regex::new(r"[ \t]+([,.!?;:])").unwrap());
    let t = spaces.replace_all(text, " ");
    let t = before.replace_all(&t, "$1");
    let lines: Vec<&str> = t.split('\n').map(|l| l.trim()).collect();
    let mut out = lines.join("\n").trim_matches(|c: char| c == ' ' || c == ',').to_string();
    // Первая буква — заглавная
    if let Some(f) = out.chars().next() {
        if f.is_lowercase() {
            out = f.to_uppercase().collect::<String>() + &out[f.len_utf8()..];
        }
    }
    out
}

/// Whisper иногда «слышит» в тишине титры и пустышки — их не вставляем.
pub fn is_hallucination(text: &str) -> bool {
    let t = text.trim().trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    t.is_empty()
        || [
            "продолжение следует",
            "субтитры сделал dimatorzok",
            "субтитры создавал dimatorzok",
            "редактор субтитров а.семкин корректор а.егорова",
            "спасибо за просмотр",
            "thank you",
            "thanks for watching",
            "you",
        ]
        .iter()
        .any(|h| t == *h || (t.starts_with("субтитры") && t.len() < 80))
}

pub fn process(raw: &str, s: &Settings) -> String {
    let mut text = raw.trim().to_string();
    if s.remove_fillers {
        text = remove_fillers(&text);
    }
    for r in s.dictionary.rules.iter().filter(|r| r.enabled) {
        text = match r.kind.as_str() {
            "command" if s.voice_commands => apply_command(&text, &r.from, &r.to),
            "command" => text,
            _ => apply_replace(&text, &r.from, &r.to),
        };
    }
    if !s.auto_punctuation {
        text = strip_punctuation(&text);
    }
    tidy(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st() -> Settings {
        Settings::default()
    }

    #[test]
    fn newline_command() {
        let out = process("Привет. Новая строка. Как дела?", &st());
        assert_eq!(out, "Привет.\nКак дела?");
    }

    #[test]
    fn paragraph_command() {
        let out = process("Первый пункт, новый абзац, второй пункт.", &st());
        assert_eq!(out, "Первый пункт\n\nВторой пункт.");
    }

    #[test]
    fn fillers_removed() {
        let out = process("Ну, короче, завтра созвон, э-э, в семь.", &st());
        assert_eq!(out, "Завтра созвон, в семь.");
    }

    #[test]
    fn replacement() {
        let mut s = st();
        s.dictionary.rules.push(crate::settings::Rule {
            id: "x".into(),
            from: "кубер".into(),
            to: "Kubernetes".into(),
            kind: "replace".into(),
            enabled: true,
        });
        assert_eq!(process("Задеплоить в кубер сегодня", &s), "Задеплоить в Kubernetes сегодня");
        // не трогаем внутри других слов
        assert_eq!(process("Куберстак", &s), "Куберстак");
    }

    #[test]
    fn hallucinations() {
        assert!(is_hallucination("Продолжение следует..."));
        assert!(is_hallucination("  "));
        assert!(!is_hallucination("Привет"));
    }
}
