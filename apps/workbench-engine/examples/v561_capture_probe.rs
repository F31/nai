//! Explicit zero-model capture fixture. Never a production avatar or readiness proof.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use minifb::{Window, WindowOptions};
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };
    let dir =
        PathBuf::from(std::env::var_os("NAI_CAPTURE_PROBE_ARTIFACTS").ok_or("artifacts required")?);
    std::fs::create_dir_all(&dir)?;
    let mut window = Window::new(
        "nai · 捕获诊断：滚动色条（非数字人）",
        768,
        768,
        WindowOptions::default(),
    )?;
    window.set_position(80, 80);
    window.set_target_fps(30);
    let mut screen = vec![0x004488aa; 768 * 768];
    window.update_with_buffer(&screen, 768, 768)?;
    std::fs::write(
        dir.join("mixed-ready.json"),
        b"{\"scope\":\"capture_fixture_not_avatar\"}",
    )?;
    let ready = Instant::now();
    while !dir.join("obs.ready").exists() {
        if ready.elapsed() > Duration::from_secs(30) {
            return Err("recorder handshake timeout".into());
        }
        window.update_with_buffer(&screen, 768, 768)?;
    }
    let start = Instant::now();
    let mut cover: Option<Window> = None;
    let mut frames = 0u64;
    while start.elapsed() < Duration::from_secs(30) && window.is_open() {
        let t = start.elapsed().as_secs_f64();
        if (10.0..20.0).contains(&t) && cover.is_none() {
            let mut w = Window::new(
                "nai · 遮挡诊断：灰色窗口（非故障）",
                800,
                810,
                WindowOptions {
                    topmost: true,
                    ..WindowOptions::default()
                },
            )?;
            w.set_position(65, 60);
            cover = Some(w);
        } else if t >= 20.0 {
            cover = None;
        }
        for (i, p) in screen.iter_mut().enumerate() {
            let band = ((i % 768) as u64 + frames * 8) / 64;
            *p = if band.is_multiple_of(2) {
                0x0033aa77
            } else {
                0x00cc6644
            };
        }
        window.update_with_buffer(&screen, 768, 768)?;
        if let Some(w) = cover.as_mut() {
            w.update_with_buffer(&vec![0x00777777; 800 * 810], 800, 810)?;
        }
        frames += 1;
    }
    std::fs::write(
        dir.join("mixed-complete.json"),
        serde_json::to_vec(
            &serde_json::json!({"scope":"capture_fixture_not_avatar","frames":frames,"elapsed_ms":start.elapsed().as_millis(),"occlusion_seconds":[10,20]}),
        )?,
    )?;
    let done = Instant::now();
    while !dir.join("obs.stopped").exists() && done.elapsed() < Duration::from_secs(15) {
        window.update_with_buffer(&screen, 768, 768)?;
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows-only capture diagnostic");
    std::process::exit(1);
}
