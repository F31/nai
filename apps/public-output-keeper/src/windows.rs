use nai_contracts::{
    audition_wire as wire,
    workbench::{PublicOutputLease, PublicOutputReason as Reason},
};
use nai_runtime_core::public_standby::StandbyGate;
use std::{
    path::{Component, Path},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
        mpsc,
    },
    time::Instant,
};
fn marker(path: &Path) -> Option<String> {
    if !path.is_absolute()
        || path.components().any(|c| matches!(c, Component::ParentDir))
        || path.file_name()?.to_str()? != "public-output.stop"
    {
        return None;
    }
    let mut prefix = std::path::PathBuf::new();
    for c in path.components() {
        prefix.push(c);
        match std::fs::symlink_metadata(&prefix) {
            Ok(m) => {
                use std::os::windows::fs::MetadataExt;
                if m.file_type().is_symlink() || m.file_attributes() & 0x400 != 0 {
                    return None;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return None,
        }
    }
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(String::new()),
        Ok(m) if m.is_file() && m.len() <= 64 => {
            let s = std::fs::read_to_string(path).ok()?;
            if s.is_empty()
                || s.len() == 36
                    && s.bytes().enumerate().all(|(i, b)| {
                        if [8, 13, 18, 23].contains(&i) {
                            b == b'-'
                        } else {
                            b.is_ascii_hexdigit()
                        }
                    })
            {
                Some(s)
            } else {
                None
            }
        }
        _ => None,
    }
}
// Fixed standby labels layered over the independently licensed avatar artwork.
fn frame(avatar: &nai_media_core::public_avatar::BuiltinAvatar) -> Vec<u32> {
    let mut p = avatar.idle_pixels().to_vec();
    for y in 44..244 {
        for x in 38..474 {
            if !(48..=239).contains(&y) || !(42..=469).contains(&x) {
                p[y * 512 + x] = 0x3b82f6;
            }
        }
    }
    fn glyph(c: char) -> [u8; 7] {
        match c {
            'N' => [17, 25, 25, 21, 19, 19, 17],
            'A' => [14, 17, 17, 31, 17, 17, 17],
            'I' => [31, 4, 4, 4, 4, 4, 31],
            'S' => [15, 16, 16, 14, 1, 1, 30],
            'T' => [31, 4, 4, 4, 4, 4, 4],
            'D' => [30, 17, 17, 17, 17, 17, 30],
            'B' => [30, 17, 17, 30, 17, 17, 30],
            'Y' => [17, 17, 10, 4, 4, 4, 4],
            _ => [0; 7],
        }
    }
    for (text, left, top, scale) in [("NAI", 54, 65, 3), ("STANDBY", 171, 253, 4)] {
        for (i, c) in text.chars().enumerate() {
            for (y, row) in glyph(c).iter().enumerate() {
                for x in 0..5 {
                    if row & (1 << (4 - x)) != 0 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                p[(top + y * scale + dy) * 512
                                    + left
                                    + i * 6 * scale
                                    + x * scale
                                    + dx] = 0xe5e7eb;
                            }
                        }
                    }
                }
            }
        }
    }
    p
}
pub(super) fn run() -> Result<(), ()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 || args[0] != "--standby" || args[1] != "--stop-file" {
        return Err(());
    }
    let path = std::path::PathBuf::from(&args[2]);
    let initial = marker(&path).ok_or(())?;
    let token = std::env::var("NAI_PUBLIC_OUTPUT_TOKEN").map_err(|_| ())?;
    let output_id = std::env::var("NAI_PUBLIC_OUTPUT_ID").map_err(|_| ())?;
    nai_runtime_core::foundation::uuid(&output_id).map_err(|_| ())?;
    let mut gate = StandbyGate::new(token).ok_or(())?;
    let mut window =
        minifb::Window::new("nai · 公共输出", 512, 288, minifb::WindowOptions::default())
            .map_err(|_| ())?;
    #[cfg(debug_assertions)]
    if std::env::var_os("NAI_PUBLIC_OUTPUT_TEST_HIDDEN").is_some() {
        window.set_position(-20000, -20000);
    }
    // Control/lease acknowledgments must not wait for the 30 Hz visual timer.
    window.set_target_fps(0);
    let avatar = nai_media_core::public_avatar::BuiltinAvatar::new(&|| Ok(())).map_err(|_| ())?;
    let visual = avatar.observe(&|| Ok(())).map_err(|_| ())?;
    let pixels = frame(&avatar);
    let mut preloader =
        crate::preload::Preloader::new(path.parent().ok_or(())?.to_path_buf(), output_id.clone());
    let mut prepared: Option<(nai_contracts::workbench::PublicWindowPrepared, Instant)> = None;
    let mut presentation: Option<Box<nai_contracts::workbench::PublicPresentationStatus>> = None;
    let status =
        |gate: &mut StandbyGate,
         now,
         frames,
         prepared: &Option<(nai_contracts::workbench::PublicWindowPrepared, Instant)>,
         presentation: &Option<Box<nai_contracts::workbench::PublicPresentationStatus>>| {
            let mut s = gate.status(now, frames);
            s.output_id = output_id.clone();
            s.presentation = presentation.clone();
            s.window_prepared = prepared.as_ref().map(|(v, _)| Box::new(v.clone()));
            if !s.terminal
                && prepared
                    .as_ref()
                    .is_some_and(|(p, _)| p.playing && !p.execution_stopped)
            {
                s.standby = false;
                s.live_ready = true;
                s.audio_active = true;
                s.reason = nai_contracts::workbench::PublicOutputReason::MediaActive as i32;
            }
            s.visual = Some(visual.clone());
            s.visual_name = nai_media_core::public_avatar::NAME.into();
            s.visual_license = nai_media_core::public_avatar::LICENSE_ID.into();
            s
        };
    window
        .update_with_buffer(&pixels, 512, 288)
        .map_err(|_| ())?;
    let mut frames = 1;
    let mut next_frame = Instant::now();
    let fault = Arc::new(AtomicU8::new(0));
    let f = fault.clone();
    let (tx, rx) = mpsc::sync_channel(4);
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            match wire::read::<PublicOutputLease>(&mut input) {
                Ok(r) => {
                    if tx.try_send(r).is_err() {
                        f.store(2, Ordering::Release);
                        break;
                    }
                }
                Err(e) => {
                    f.store(
                        if e.kind() == std::io::ErrorKind::InvalidData {
                            2
                        } else {
                            1
                        },
                        Ordering::Release,
                    );
                    break;
                }
            }
        }
    });
    let clock = Instant::now();
    wire::write(
        &mut std::io::stdout().lock(),
        &status(&mut gate, 0, frames, &prepared, &presentation),
    )
    .map_err(|_| ())?;
    let mut terminal_sent = false;
    while window.is_open() {
        let now = u64::try_from(clock.elapsed().as_millis()).map_err(|_| ())?;
        if marker(&path).as_ref() != Some(&initial) || window.is_key_down(minifb::Key::Escape) {
            gate.stop(Reason::Stopped);
        }
        match fault.load(Ordering::Acquire) {
            1 => gate.stop(Reason::InputClosed),
            2 => gate.stop(Reason::ProtocolRejected),
            _ => (),
        }
        gate.poll(now);
        if gate.terminal() {
            preloader.clear();
            prepared = None;
        } else {
            if let Some(v) = preloader.poll() {
                prepared = Some(v);
            }
            if prepared.as_ref().is_some_and(|(view, until)| {
                Instant::now() >= *until || view.device_ready && !preloader.current_valid()
            }) {
                preloader.clear();
                prepared = None;
            }
        }
        if presentation.is_some() && (gate.terminal() || preloader.frame().is_none()) {
            window
                .update_with_buffer(&pixels, 512, 288)
                .map_err(|_| ())?;
            frames = frames.saturating_add(1);
            presentation = None;
        }
        if !gate.terminal() {
            while let Ok(r) = rx.try_recv() {
                if gate.accept(&r, now) {
                    if r.clear_prepared {
                        preloader.clear();
                        prepared = None;
                    }
                    if let Some(x) = r.execution
                        && preloader.execute(*x).is_err()
                    {
                        gate.stop(Reason::ProtocolRejected);
                    }
                    if r.refresh_execution_challenge && preloader.refresh().is_err() {
                        gate.stop(Reason::ProtocolRejected);
                    }
                    if let Some(x) = r.prepare {
                        prepared = None;
                        if preloader.prepare(*x).is_err() {
                            gate.stop(Reason::ProtocolRejected);
                        }
                    }
                }
                if gate.terminal() {
                    preloader.clear();
                    prepared = None;
                }
                if presentation.is_some() && (gate.terminal() || prepared.is_none()) {
                    window
                        .update_with_buffer(&pixels, 512, 288)
                        .map_err(|_| ())?;
                    frames = frames.saturating_add(1);
                    presentation = None;
                }
                let state = status(&mut gate, now, frames, &prepared, &presentation);
                if wire::write(&mut std::io::stdout().lock(), &state).is_err() {
                    gate.stop(Reason::InputClosed);
                }
                if gate.terminal() {
                    terminal_sent = true;
                    break;
                }
            }
        }
        if gate.terminal() && !terminal_sent {
            let _ = wire::write(
                &mut std::io::stdout().lock(),
                &status(&mut gate, now, frames, &prepared, &presentation),
            );
            terminal_sent = true;
        }
        if Instant::now() >= next_frame {
            let active_frame = if gate.terminal() {
                None
            } else {
                preloader.frame()
            };
            window
                .update_with_buffer(
                    active_frame
                        .as_ref()
                        .map_or(pixels.as_slice(), |f| f.pixels.as_slice()),
                    512,
                    288,
                )
                .map_err(|_| ())?;
            frames = frames.saturating_add(1);
            presentation = active_frame.as_ref().map(|f| {
                Box::new(nai_contracts::workbench::PublicPresentationStatus {
                    preparation_id: f.preparation_id.clone(),
                    frame_index: f.base.frame_index,
                    sample_start: f.base.sample_start,
                    caption_visible: f.caption_visible,
                    caption_sha256: f.caption_sha.clone(),
                    composited_sha256: f.sha.clone(),
                    presentation_count: frames,
                    submission_clock: true,
                })
            });
            next_frame = Instant::now() + std::time::Duration::from_millis(33);
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    Ok(())
}
