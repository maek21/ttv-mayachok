//! Разбор голосовых команд-напоминаний на русском без сети и LLM.
//!
//! «напомни через 20 минут выключить духовку», «маячок, завтра в семь вечера созвон»,
//! «срочно: в пол четвёртого отправить отчёт», «каждый понедельник в 10 планёрка».

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    None,
    Daily,
    Weekdays,
    Weekly,
    Monthly,
}

impl Repeat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Repeat::None => "none",
            Repeat::Daily => "daily",
            Repeat::Weekdays => "weekdays",
            Repeat::Weekly => "weekly",
            Repeat::Monthly => "monthly",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "daily" => Repeat::Daily,
            "weekdays" => Repeat::Weekdays,
            "weekly" => Repeat::Weekly,
            "monthly" => Repeat::Monthly,
            _ => Repeat::None,
        }
    }

    /// Следующее срабатывание после `from` (для повторяющихся)
    pub fn next_after(&self, due: NaiveDateTime, from: NaiveDateTime) -> Option<NaiveDateTime> {
        let mut d = due;
        let step = |d: NaiveDateTime| -> NaiveDateTime {
            match self {
                Repeat::Daily => d + Duration::days(1),
                Repeat::Weekly => d + Duration::days(7),
                Repeat::Weekdays => {
                    let mut n = d + Duration::days(1);
                    while matches!(n.weekday(), Weekday::Sat | Weekday::Sun) {
                        n += Duration::days(1);
                    }
                    n
                }
                Repeat::Monthly => add_month(d),
                Repeat::None => d,
            }
        };
        if *self == Repeat::None {
            return None;
        }
        loop {
            d = step(d);
            if d > from {
                return Some(d);
            }
        }
    }
}

fn add_month(d: NaiveDateTime) -> NaiveDateTime {
    let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
    let mut day = d.day();
    loop {
        if let Some(nd) = NaiveDate::from_ymd_opt(y, m, day) {
            return nd.and_time(d.time());
        }
        day -= 1;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub text: String,
    pub due: NaiveDateTime,
    pub urgent: bool,
    pub repeat: Repeat,
}

#[derive(Clone)]
struct Tok {
    orig: String,
    low: String,
}

fn tokenize(s: &str) -> Vec<Tok> {
    s.split_whitespace()
        .map(|w| {
            let t = w.trim_matches(|c: char| !(c.is_alphanumeric() || c == '-' || c == ':' || c == '.'))
                .trim_matches(|c: char| c == '.' || c == '-' || c == ':');
            Tok { orig: t.to_string(), low: t.to_lowercase().replace('ё', "е") }
        })
        .filter(|t| !t.low.is_empty())
        .collect()
}

fn num_word(w: &str) -> Option<f64> {
    Some(match w {
        "ноль" => 0.0,
        "один" | "одна" | "одну" | "одного" => 1.0,
        "два" | "две" | "пару" | "пара" => 2.0,
        "три" => 3.0,
        "четыре" => 4.0,
        "пять" => 5.0,
        "шесть" => 6.0,
        "семь" => 7.0,
        "восемь" => 8.0,
        "девять" => 9.0,
        "десять" => 10.0,
        "одиннадцать" => 11.0,
        "двенадцать" => 12.0,
        "тринадцать" => 13.0,
        "четырнадцать" => 14.0,
        "пятнадцать" => 15.0,
        "шестнадцать" => 16.0,
        "семнадцать" => 17.0,
        "восемнадцать" => 18.0,
        "девятнадцать" => 19.0,
        "двадцать" => 20.0,
        "тридцать" => 30.0,
        "сорок" => 40.0,
        "пятьдесят" => 50.0,
        "полтора" | "полторы" => 1.5,
        _ => return None,
    })
}

/// Число из токенов начиная с i: «25», «двадцать пять», «полтора». Возвращает (значение, сколько токенов)
fn number_at(t: &[Tok], i: usize) -> Option<(f64, usize)> {
    let w = &t.get(i)?.low;
    if let Ok(n) = w.replace(',', ".").parse::<f64>() {
        return Some((n, 1));
    }
    let n = num_word(w)?;
    // «двадцать пять»
    if n >= 20.0 && n % 10.0 == 0.0 {
        if let Some(u) = t.get(i + 1).and_then(|x| num_word(&x.low)) {
            if u < 10.0 && u >= 1.0 {
                return Some((n + u, 2));
            }
        }
    }
    Some((n, 1))
}

fn ordinal_gen(w: &str) -> Option<u32> {
    Some(match w {
        "первого" => 1,
        "второго" => 2,
        "третьего" => 3,
        "четвертого" => 4,
        "пятого" => 5,
        "шестого" => 6,
        "седьмого" => 7,
        "восьмого" => 8,
        "девятого" => 9,
        "десятого" => 10,
        "одиннадцатого" => 11,
        "двенадцатого" => 12,
        _ => return None,
    })
}

fn weekday(w: &str) -> Option<Weekday> {
    Some(match w {
        "понедельник" | "понедельникам" => Weekday::Mon,
        "вторник" | "вторникам" => Weekday::Tue,
        "среду" | "среда" | "средам" => Weekday::Wed,
        "четверг" | "четвергам" => Weekday::Thu,
        "пятницу" | "пятница" | "пятницам" => Weekday::Fri,
        "субботу" | "суббота" | "субботам" => Weekday::Sat,
        "воскресенье" | "воскресеньям" => Weekday::Sun,
        _ => return None,
    })
}

fn month(w: &str) -> Option<u32> {
    Some(match w {
        "января" => 1,
        "февраля" => 2,
        "марта" => 3,
        "апреля" => 4,
        "мая" => 5,
        "июня" => 6,
        "июля" => 7,
        "августа" => 8,
        "сентября" => 9,
        "октября" => 10,
        "ноября" => 11,
        "декабря" => 12,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Meridiem {
    Am,
    Day,
    Pm,
    Night,
}

fn meridiem(w: &str) -> Option<Meridiem> {
    Some(match w {
        "утра" => Meridiem::Am,
        "дня" => Meridiem::Day,
        "вечера" => Meridiem::Pm,
        "ночи" => Meridiem::Night,
        _ => return None,
    })
}

fn apply_meridiem(h: u32, m: Option<Meridiem>) -> u32 {
    match m {
        Some(Meridiem::Am) => {
            if h == 12 {
                0
            } else {
                h
            }
        }
        Some(Meridiem::Day) | Some(Meridiem::Pm) => {
            if h < 12 {
                h + 12
            } else {
                h
            }
        }
        Some(Meridiem::Night) => {
            if h >= 6 && h < 12 {
                h + 12
            } else if h == 12 {
                0
            } else {
                h
            }
        }
        None => h,
    }
}

/// «19:00», «19.30», «7» → (часы, минуты?)
fn clock_token(w: &str) -> Option<(u32, Option<u32>)> {
    let sep = w.find([':', '.']);
    if let Some(p) = sep {
        let h: u32 = w[..p].parse().ok()?;
        let m: u32 = w[p + 1..].parse().ok()?;
        if h < 24 && m < 60 {
            return Some((h, Some(m)));
        }
        return None;
    }
    let h: u32 = w.parse().ok()?;
    (h < 24).then_some((h, None))
}

const TRIGGERS_DEFAULT: [&str; 3] = ["напомни", "маячок", "срочно"];

fn is_trigger(w: &str, triggers: &[String]) -> bool {
    let list: Vec<String> = if triggers.is_empty() {
        TRIGGERS_DEFAULT.iter().map(|s| s.to_string()).collect()
    } else {
        triggers.iter().map(|s| s.to_lowercase().replace('ё', "е")).collect()
    };
    list.iter().any(|t| {
        w == t
            || (t == "напомни" && matches!(w, "напомнить" | "напомните" | "напоминание" | "напомню"))
            || (t == "маячок" && matches!(w, "маячка" | "маячку"))
            || (t == "срочно" && matches!(w, "срочное" | "срочный" | "срочная"))
    })
}

fn is_urgent_word(w: &str) -> bool {
    matches!(w, "срочно" | "срочное" | "срочный" | "срочная" | "важно")
}

/// Разобрать фразу. `None` — это не команда, а обычный текст для вставки.
pub fn parse(raw: &str, triggers: &[String], now: NaiveDateTime, default_hour: u32) -> Option<Parsed> {
    let toks = tokenize(raw);
    if toks.is_empty() {
        return None;
    }
    // Триггер в первых словах: «эй маячок», «так, напомни», «поставь маячок»
    let fillers = ["эй", "так", "ну", "слушай", "поставь", "сделай", "создай", "окей", "ок"];
    let mut start = 0;
    while start < toks.len().min(3) && fillers.contains(&toks[start].low.as_str()) {
        start += 1;
    }
    if start >= toks.len() || !is_trigger(&toks[start].low, triggers) {
        return None;
    }
    let mut urgent = is_urgent_word(&toks[start].low);
    let mut used = vec![false; toks.len()];
    for u in used.iter_mut().take(start + 1) {
        *u = true;
    }
    // «маячок, срочно», «напомни срочно»
    let mut j = start + 1;
    while j < toks.len().min(start + 3) {
        let w = toks[j].low.as_str();
        if is_urgent_word(w) {
            urgent = true;
            used[j] = true;
        } else if is_trigger(w, triggers) || matches!(w, "мне" | "пожалуйста" | "нам") {
            used[j] = true;
        } else {
            break;
        }
        j += 1;
    }

    let today = now.date();
    let mut rel: Option<Duration> = None;
    let mut date: Option<NaiveDate> = None;
    let mut date_explicit_weekday = false;
    let mut time: Option<(u32, u32)> = None;
    let mut time_ambiguous = false;
    // Время сказано без минут через двоеточие («в 9», «в пол девятого») — можно трактовать как вечер
    let mut time_loose = false;
    let mut repeat = Repeat::None;

    let t = &toks;
    let n = t.len();
    let mut i = start + 1;
    while i < n {
        if used[i] {
            i += 1;
            continue;
        }
        let w = t[i].low.as_str();
        let next = |k: usize| t.get(i + k).map(|x| x.low.as_str()).unwrap_or("");

        // --- «через …»
        if w == "через" {
            let mut k = i + 1;
            let mut total = Duration::zero();
            let mut ok = false;
            loop {
                let (val, len) = match t.get(k).map(|x| x.low.as_str()) {
                    Some("полчаса") => {
                        total += Duration::minutes(30);
                        ok = true;
                        k += 1;
                        continue;
                    }
                    Some("час") | Some("минуту") | Some("день") | Some("неделю") | Some("секунду") => (1.0, 0),
                    _ => match number_at(t, k) {
                        Some(v) => v,
                        None => break,
                    },
                };
                let unit = t.get(k + len).map(|x| x.low.as_str()).unwrap_or("");
                let mins = match unit {
                    u if u.starts_with("мин") => 1.0,
                    u if u.starts_with("час") => 60.0,
                    u if u.starts_with("сек") => 1.0 / 60.0,
                    "день" | "дня" | "дней" | "сутки" | "суток" => 1440.0,
                    u if u.starts_with("недел") => 10080.0,
                    _ => break,
                };
                total += Duration::seconds((val * mins * 60.0).round() as i64);
                ok = true;
                k += len + 1;
                if t.get(k).map(|x| x.low == "и").unwrap_or(false) {
                    k += 1;
                }
            }
            if ok {
                rel = Some(total);
                for u in used.iter_mut().take(k).skip(i) {
                    *u = true;
                }
                i = k;
                continue;
            }
        }

        // --- повторы
        if matches!(w, "каждый" | "каждое" | "каждую" | "каждого" | "ежедневно" | "по") {
            let nw = next(1);
            let mut hit = true;
            let mut len = 2;
            match (w, nw) {
                ("ежедневно", _) => {
                    repeat = Repeat::Daily;
                    len = 1;
                }
                (_, "день") | (_, "дням") => repeat = Repeat::Daily,
                (_, "утро") | (_, "утрам") => {
                    repeat = Repeat::Daily;
                    time.get_or_insert((9, 0));
                }
                (_, "вечер") | (_, "вечерам") => {
                    repeat = Repeat::Daily;
                    time.get_or_insert((19, 0));
                }
                (_, "будням") | (_, "будний") => repeat = Repeat::Weekdays,
                (_, "неделю") => repeat = Repeat::Weekly,
                (_, "месяц") => repeat = Repeat::Monthly,
                (_, x) if weekday(x).is_some() => {
                    repeat = Repeat::Weekly;
                    let wd = weekday(x).unwrap();
                    date = Some(next_weekday(today, wd, true));
                    date_explicit_weekday = true;
                }
                _ => hit = false,
            }
            if hit {
                for u in used.iter_mut().skip(i).take(len) {
                    *u = true;
                }
                i += len;
                continue;
            }
        }

        // --- дни
        match w {
            "сегодня" => {
                date = Some(today);
                used[i] = true;
                i += 1;
                continue;
            }
            "завтра" => {
                date = Some(today + Duration::days(1));
                used[i] = true;
                i += 1;
                continue;
            }
            "послезавтра" => {
                date = Some(today + Duration::days(2));
                used[i] = true;
                i += 1;
                continue;
            }
            "утром" | "днем" | "вечером" | "ночью" => {
                let h = match w {
                    "утром" => 9,
                    "днем" => 13,
                    "вечером" => 19,
                    _ => 23,
                };
                time.get_or_insert((h, 0));
                used[i] = true;
                i += 1;
                continue;
            }
            "в" | "во" | "на" => {
                let nw = next(1);
                // «в эту/следующую пятницу»
                let (skip, wdw) = if matches!(nw, "эту" | "этот" | "следующую" | "следующий" | "ближайшую" | "ближайший") {
                    (2, next(2))
                } else {
                    (1, nw)
                };
                if let Some(wd) = weekday(wdw) {
                    let later = matches!(nw, "следующую" | "следующий");
                    let mut d = next_weekday(today, wd, false);
                    if later && d - today < Duration::days(7) {
                        d += Duration::days(7);
                    }
                    date = Some(d);
                    date_explicit_weekday = true;
                    for u in used.iter_mut().skip(i).take(skip + 1) {
                        *u = true;
                    }
                    i += skip + 1;
                    continue;
                }
                if w == "на" {
                    // «на завтра», «на 5 октября»
                    i += 1;
                    continue;
                }
                // «в полдень», «в полночь»
                if nw == "полдень" || nw == "полночь" {
                    time = Some((if nw == "полдень" { 12 } else { 0 }, 0));
                    used[i] = true;
                    used[i + 1] = true;
                    i += 2;
                    continue;
                }
                // «в пол четвертого», «в половине четвертого», «в полчетвертого»
                let (half_len, ord) = if matches!(nw, "пол" | "половине" | "половина") {
                    (2, ordinal_gen(next(2)))
                } else if let Some(rest) = nw.strip_prefix("пол") {
                    (1, ordinal_gen(rest))
                } else {
                    (0, None)
                };
                if let Some(o) = ord {
                    let mut h = if o == 1 { 0 } else { o - 1 };
                    let mut len = 1 + half_len;
                    let mer = meridiem(t.get(i + len).map(|x| x.low.as_str()).unwrap_or(""));
                    if mer.is_some() {
                        len += 1;
                        h = apply_meridiem(h, mer);
                    } else {
                        time_ambiguous = true;
                        time_loose = true;
                    }
                    time = Some((h, 30));
                    for u in used.iter_mut().skip(i).take(len) {
                        *u = true;
                    }
                    i += len;
                    continue;
                }
                // «в 19:00», «в 7», «в 7 30», «в семь», «в семь тридцать», «в 7 вечера»
                let mut k = i + 1;
                let mut hm: Option<(u32, Option<u32>)> = clock_token(nw).map(|x| {
                    k += 1;
                    x
                });
                let had_sep = hm.map(|x| x.1.is_some()).unwrap_or(false);
                if hm.is_none() {
                    if let Some((v, len)) = number_at(t, i + 1) {
                        if v.fract() == 0.0 && v < 24.0 && !nw.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                            hm = Some((v as u32, None));
                            k += len;
                        }
                    }
                }
                if let Some((h0, mut m0)) = hm {
                    // Не время, а дата: «в 5 октября» — обработаем ниже
                    if month(t.get(k).map(|x| x.low.as_str()).unwrap_or("")).is_none() {
                        if m0.is_none() {
                            // «в 7 30», «в семь тридцать», «в 7 часов 30 минут»
                            if t.get(k).map(|x| x.low.starts_with("час")).unwrap_or(false) {
                                k += 1;
                            }
                            if let Some((mv, len)) = number_at(t, k) {
                                let unit_ok = t.get(k + len).map(|x| x.low.starts_with("мин")).unwrap_or(false);
                                if mv.fract() == 0.0 && mv < 60.0 && (mv >= 10.0 || unit_ok) {
                                    m0 = Some(mv as u32);
                                    k += len;
                                    if unit_ok {
                                        k += 1;
                                    }
                                }
                            }
                        }
                        let mer = meridiem(t.get(k).map(|x| x.low.as_str()).unwrap_or(""));
                        if mer.is_some() {
                            k += 1;
                        }
                        let h = apply_meridiem(h0, mer);
                        time_ambiguous = mer.is_none() && h0 <= 12;
                        time_loose = time_ambiguous && !had_sep;
                        time = Some((h, m0.unwrap_or(0)));
                        for u in used.iter_mut().take(k).skip(i) {
                            *u = true;
                        }
                        i = k;
                        continue;
                    }
                }
            }
            _ => {}
        }

        // --- «5 октября», «на 5 октября»
        if let Some((d, len)) = number_at(t, i) {
            if let Some(m) = month(t.get(i + len).map(|x| x.low.as_str()).unwrap_or("")) {
                let mut y = today.year();
                if let Some(mut nd) = NaiveDate::from_ymd_opt(y, m, d as u32) {
                    if nd < today {
                        y += 1;
                        nd = NaiveDate::from_ymd_opt(y, m, d as u32).unwrap_or(nd);
                    }
                    date = Some(nd);
                    for u in used.iter_mut().skip(i).take(len + 1) {
                        *u = true;
                    }
                    // «в 5 октября» — съедаем предлог
                    if i > 0 && matches!(t[i - 1].low.as_str(), "в" | "на" | "к") {
                        used[i - 1] = true;
                    }
                    i += len + 1;
                    continue;
                }
            }
        }
        i += 1;
    }

    // --- время срабатывания
    let due = if let Some(d) = rel {
        now + d
    } else {
        let explicit_date = date.is_some();
        let d = date.unwrap_or(today);
        let (mut h, m) = time.unwrap_or(if explicit_date || repeat != Repeat::None {
            (default_hour, 0)
        } else {
            // Ни даты, ни времени — через час
            let at = now + Duration::hours(1);
            (at.hour_(), at.minute_())
        });
        // «в 7» без «утра/вечера»: ранние часы — скорее вечер
        if time_ambiguous && (1..=7).contains(&h) {
            h += 12;
        }
        let mut dt = d.and_time(NaiveTime::from_hms_opt(h, m, 0).unwrap_or_default());
        if time_loose && dt <= now && !explicit_date && h < 12 && d.and_hms_opt(h + 12, m, 0).map(|x| x > now).unwrap_or(false) {
            // «в 9» в 10 утра — значит 21:00
            dt = d.and_hms_opt(h + 12, m, 0).unwrap();
        }
        if dt <= now {
            if date_explicit_weekday {
                dt += Duration::days(7);
            } else if !explicit_date || repeat != Repeat::None {
                dt += Duration::days(1);
                if repeat == Repeat::Weekdays {
                    while matches!(dt.weekday(), Weekday::Sat | Weekday::Sun) {
                        dt += Duration::days(1);
                    }
                }
            }
        }
        if repeat == Repeat::Weekdays {
            while matches!(dt.weekday(), Weekday::Sat | Weekday::Sun) {
                dt += Duration::days(1);
            }
        }
        dt
    };

    // --- текст: что осталось, без служебных слов по краям
    let mut words: Vec<&str> = t.iter().zip(used.iter()).filter(|(_, u)| !**u).map(|(x, _)| x.orig.as_str()).collect();
    let lead = ["что", "чтобы", "о", "об", "про", "том", "мне", "нам", "надо", "пожалуйста", "и", "а", "в", "на"];
    while let Some(f) = words.first() {
        if lead.contains(&f.to_lowercase().as_str()) {
            words.remove(0);
        } else {
            break;
        }
    }
    while let Some(l) = words.last() {
        if matches!(l.to_lowercase().as_str(), "пожалуйста" | "в" | "на" | "и" | "к") {
            words.pop();
        } else {
            break;
        }
    }
    let mut text = words.join(" ");
    if text.is_empty() {
        text = "Напоминание".into();
    }
    let mut c = text.chars();
    let text = match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => text,
    };
    Some(Parsed { text, due, urgent, repeat })
}

fn next_weekday(from: NaiveDate, wd: Weekday, allow_today: bool) -> NaiveDate {
    let mut d = from;
    if !allow_today {
        // Сегодняшний день недели оставляем: поздно ли — решит проверка времени ниже
    }
    for _ in 0..7 {
        if d.weekday() == wd {
            return d;
        }
        d += Duration::days(1);
    }
    d
}

trait HM {
    fn hour_(&self) -> u32;
    fn minute_(&self) -> u32;
}
impl HM for NaiveDateTime {
    fn hour_(&self) -> u32 {
        chrono::Timelike::hour(self)
    }
    fn minute_(&self) -> u32 {
        chrono::Timelike::minute(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Понедельник, 28 сентября 2026, 14:02
    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 28).unwrap().and_hms_opt(14, 2, 0).unwrap()
    }
    fn p(s: &str) -> Parsed {
        parse(s, &[], now(), 9).unwrap_or_else(|| panic!("не распознал: {s}"))
    }
    fn at(d: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, if d < 28 { 10 } else { 9 }, d).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn not_a_command() {
        assert!(parse("Завтра в семь созвон", &[], now(), 9).is_none());
        assert!(parse("Маячки это круто", &[], now(), 9).is_none());
    }

    #[test]
    fn relative() {
        let r = p("Напомни через 20 минут выключить духовку.");
        assert_eq!(r.text, "Выключить духовку");
        assert_eq!(r.due, now() + Duration::minutes(20));
        assert_eq!(p("напомни через полчаса позвонить маме").due, now() + Duration::minutes(30));
        assert_eq!(p("напомни через полтора часа выйти").due, now() + Duration::minutes(90));
        assert_eq!(p("напомни через час выйти").due, now() + Duration::hours(1));
        assert_eq!(p("напомни через два часа 15 минут проверить").due, now() + Duration::minutes(135));
        assert_eq!(p("напомни через двадцать пять минут чай").due, now() + Duration::minutes(25));
    }

    #[test]
    fn tomorrow_evening() {
        let r = p("Маячок, завтра в семь вечера созвон по билду.");
        assert_eq!(r.text, "Созвон по билду");
        assert_eq!(r.due, at(29, 19, 0));
        assert!(!r.urgent);
        // «в 7» без уточнения — вечер
        assert_eq!(p("маячок завтра в 7 созвон").due, at(29, 19, 0));
        assert_eq!(p("маячок завтра в 7 утра пробежка").due, at(29, 7, 0));
    }

    #[test]
    fn urgent_half_past() {
        let r = p("Срочно, в пол четвёртого отправить отчёт.");
        assert!(r.urgent);
        assert_eq!(r.text, "Отправить отчёт");
        assert_eq!(r.due, at(28, 15, 30));
        assert_eq!(p("напомни срочно в полчетвертого отчёт").due, at(28, 15, 30));
        assert!(p("маячок, срочно: позвонить").urgent);
    }

    #[test]
    fn clock_formats() {
        assert_eq!(p("напомни в 19:00 созвон").due, at(28, 19, 0));
        assert_eq!(p("напомни в 15.30 отчёт").due, at(28, 15, 30));
        assert_eq!(p("напомни в 16 45 забрать").due, at(28, 16, 45));
        // уже прошло сегодня → завтра
        assert_eq!(p("напомни в 10:00 планёрка").due, at(29, 10, 0));
        // «в 9» в 14:02 — это 21:00
        assert_eq!(p("напомни в 9 полить цветы").due, at(28, 21, 0));
        assert_eq!(p("напомни в полдень обед").due, at(29, 12, 0));
    }

    #[test]
    fn weekdays_and_dates() {
        let r = p("напомни в пятницу купить подарок Лёше");
        assert_eq!(r.text, "Купить подарок Лёше");
        assert_eq!(r.due, at(2, 9, 0));
        assert_eq!(p("напомни во вторник в 18:00 спортзал").due, at(29, 18, 0));
        assert_eq!(p("напомни 5 октября оплатить интернет").due, at(5, 9, 0));
        assert_eq!(p("напомни 5 октября оплатить интернет").text, "Оплатить интернет");
        // сегодня понедельник, но 10:00 уже прошло → следующий понедельник
        assert_eq!(p("напомни в понедельник в 10 отчёт").due, at(5, 10, 0));
    }

    #[test]
    fn repeats() {
        let r = p("Маячок, каждый понедельник в 10 планёрка");
        assert_eq!(r.repeat, Repeat::Weekly);
        assert_eq!(r.text, "Планёрка");
        assert_eq!(r.due, at(5, 10, 0));
        let r = p("напомни каждое утро пить воду");
        assert_eq!(r.repeat, Repeat::Daily);
        assert_eq!(r.due, at(29, 9, 0));
        assert_eq!(p("напомни по будням в 18:30 уйти с работы").repeat, Repeat::Weekdays);
        assert_eq!(
            Repeat::Weekdays.next_after(at(2, 18, 30), at(2, 18, 31)),
            Some(at(5, 18, 30))
        );
    }

    #[test]
    fn no_time_is_in_an_hour() {
        let r = p("напомни мне что надо купить хлеб");
        assert_eq!(r.text, "Купить хлеб");
        assert_eq!(r.due, now() + Duration::hours(1));
    }

    #[test]
    fn custom_triggers() {
        let tr = vec!["эй маяк".to_string(), "маяк".to_string()];
        assert!(parse("маяк через 5 минут чайник", &tr, now(), 9).is_some());
        assert!(parse("напомни через 5 минут чайник", &tr, now(), 9).is_none());
    }
}
