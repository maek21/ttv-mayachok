//! Захват микрофона (cpal) → моно f32 → 16 кГц для whisper.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

pub const TARGET_RATE: u32 = 16_000;

pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut out = vec![];
    if let Ok(devs) = host.input_devices() {
        for d in devs {
            if let Ok(n) = d.name() {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
    }
    out
}

pub fn default_input_name() -> Option<String> {
    cpal::default_host().default_input_device().and_then(|d| d.name().ok())
}

fn find_device(name: Option<&str>) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if let Some(name) = name {
        if let Ok(mut devs) = host.input_devices() {
            if let Some(d) = devs.find(|d| d.name().map(|n| n == name).unwrap_or(false)) {
                return Some(d);
            }
        }
        log::warn!("Микрофон «{name}» не найден, беру системный по умолчанию");
    }
    host.default_input_device()
}

pub struct Capture {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    buf: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
    rate: u32,
    pub device_name: String,
}

impl Capture {
    pub fn start(device: Option<&str>) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let buf = Arc::new(Mutex::new(Vec::<f32>::with_capacity(48_000 * 30)));
        let level = Arc::new(AtomicU32::new(0));
        let (tx, rx) = mpsc::channel::<Result<(u32, String), String>>();
        let device = device.map(|s| s.to_string());

        let (s2, b2, l2) = (stop.clone(), buf.clone(), level.clone());
        // cpal::Stream не Send на Windows, поэтому живёт в своём потоке
        let handle = std::thread::Builder::new()
            .name("mic-capture".into())
            .spawn(move || {
                let dev = match find_device(device.as_deref()) {
                    Some(d) => d,
                    None => {
                        let _ = tx.send(Err("Микрофон не найден".into()));
                        return;
                    }
                };
                let name = dev.name().unwrap_or_else(|_| "Микрофон".into());
                let cfg = match dev.default_input_config() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(format!("Микрофон недоступен: {e}")));
                        return;
                    }
                };
                let rate = cfg.sample_rate().0;
                let channels = cfg.channels() as usize;
                let sc: cpal::StreamConfig = cfg.clone().into();
                let stream = match cfg.sample_format() {
                    cpal::SampleFormat::F32 => build::<f32>(&dev, &sc, channels, b2, l2),
                    cpal::SampleFormat::I16 => build::<i16>(&dev, &sc, channels, b2, l2),
                    cpal::SampleFormat::U16 => build::<u16>(&dev, &sc, channels, b2, l2),
                    cpal::SampleFormat::I32 => build::<i32>(&dev, &sc, channels, b2, l2),
                    cpal::SampleFormat::U8 => build::<u8>(&dev, &sc, channels, b2, l2),
                    f => Err(format!("Формат {f:?} не поддерживается")),
                };
                let stream = match stream {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };
                if let Err(e) = stream.play() {
                    let _ = tx.send(Err(format!("Не удалось начать запись: {e}")));
                    return;
                }
                let _ = tx.send(Ok((rate, name)));
                while !s2.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(10));
                }
                drop(stream);
            })
            .map_err(|e| e.to_string())?;

        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok((rate, device_name))) => Ok(Self { stop, handle: Some(handle), buf, level, rate, device_name }),
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => {
                stop.store(true, Ordering::Relaxed);
                Err("Микрофон не отвечает".into())
            }
        }
    }

    /// Текущий уровень (RMS последнего буфера), 0..1
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    pub fn seconds(&self) -> f32 {
        self.buf.lock().len() as f32 / self.rate as f32
    }

    /// Копия записанного звука в 16 кГц (для живого предпросмотра)
    pub fn snapshot_16k(&self) -> Vec<f32> {
        let data = self.buf.lock().clone();
        resample(&data, self.rate, TARGET_RATE)
    }

    pub fn stop(mut self) -> Vec<f32> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let data = std::mem::take(&mut *self.buf.lock());
        resample(&data, self.rate, TARGET_RATE)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn build<T>(
    dev: &cpal::Device,
    cfg: &cpal::StreamConfig,
    channels: usize,
    buf: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let err_fn = |e| log::error!("Ошибка аудиопотока: {e}");
    dev.build_input_stream(
        cfg,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let mut sum = 0.0f32;
            let mut mono = Vec::with_capacity(data.len() / channels.max(1));
            for frame in data.chunks(channels.max(1)) {
                let s = frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / frame.len() as f32;
                sum += s * s;
                mono.push(s);
            }
            if !mono.is_empty() {
                let rms = (sum / mono.len() as f32).sqrt();
                level.store(rms.to_bits(), Ordering::Relaxed);
            }
            buf.lock().extend_from_slice(&mono);
        },
        err_fn,
        None,
    )
    .map_err(|e| format!("Не удалось открыть микрофон: {e}"))
}

/// Ресемплинг: сглаживание (анти-алиасинг скользящим средним) + линейная интерполяция.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == to {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let smoothed: Vec<f32> = if ratio > 1.0 {
        let k = ratio.round().max(1.0) as usize;
        let mut acc = 0.0f32;
        let mut out = Vec::with_capacity(input.len());
        for i in 0..input.len() {
            acc += input[i];
            if i >= k {
                acc -= input[i - k];
            }
            out.push(acc / (i + 1).min(k) as f32);
        }
        out
    } else {
        input.to_vec()
    };
    let out_len = (input.len() as f64 / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let a = smoothed[idx.min(smoothed.len() - 1)];
        let b = smoothed[(idx + 1).min(smoothed.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

/// Срез низких частот (~80 Гц) и выравнивание громкости.
pub fn clean(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }
    let dt = 1.0 / TARGET_RATE as f32;
    let rc = 1.0 / (2.0 * std::f32::consts::PI * 80.0);
    let alpha = rc / (rc + dt);
    let mut prev_in = samples[0];
    let mut prev_out = 0.0f32;
    for s in samples.iter_mut() {
        let x = *s;
        let y = alpha * (prev_out + x - prev_in);
        prev_in = x;
        prev_out = y;
        *s = y;
    }
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 0.01 && peak < 0.9 {
        let g = (0.9 / peak).min(8.0);
        for s in samples.iter_mut() {
            *s *= g;
        }
    }
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

pub fn write_wav(path: &std::path::Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        w.write_sample(v).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

/// WAV в памяти (для кодирования в облако)
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cur = std::io::Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cur, spec).expect("wav writer");
        for s in samples {
            let _ = w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
        let _ = w.finalize();
    }
    cur.into_inner()
}
