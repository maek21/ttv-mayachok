use whisper_rs::*;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    install_logging_hooks();
    let n = 16000 * 3;
    let audio: Vec<f32> = (0..n).map(|i| { let t = i as f32 / 16000.0; 0.2 * (t * 3.0).sin().abs() * (2.0 * 3.14159 * 180.0 * t).sin() }).collect();
    for (flash, ac, lang) in [(true, 256, Some("auto")), (true, 384, Some("auto")), (true, 0, Some("auto")), (false, 384, Some("auto")), (true, 384, Some("ru")), (true, 256, Some("ru"))] {
        let mut cp = WhisperContextParameters::default();
        cp.use_gpu(false);
        cp.flash_attn(flash);
        let ctx = WhisperContext::new_with_params(&path, cp).unwrap();
        let mut st = ctx.create_state().unwrap();
        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        p.set_n_threads(4);
        p.set_no_timestamps(true);
        p.set_language(lang);
        p.set_audio_ctx(ac);
        p.set_single_segment(false);
        p.set_initial_prompt("Привет. Это диктовка, с запятыми и точками. Маячок.");
        let r = st.full(p, &audio);
        println!("flash={flash} ac={ac} lang={lang:?} -> {:?}", r.map(|_| "ok"));
    }
}
