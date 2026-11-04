#![cfg(windows)]
#[cfg(feature = "clocked-experiment")]
pub mod clocked_experiment;
mod cursor;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cursor::SubmissionCursor;
use nai_contracts::workbench::OutputDeviceView;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceUnavailable;
pub fn snapshot() -> Result<Vec<(cpal::Device, OutputDeviceView)>, DeviceUnavailable> {
    let host = cpal::default_host();
    let mut devices = Vec::new();
    if let Some(d) = host.default_output_device() {
        devices.push((d, true));
    }
    for d in host.output_devices().map_err(|_| DeviceUnavailable)? {
        let duplicate = devices
            .iter()
            .any(|(old, _)| match (old.as_inner(), d.as_inner()) {
                (
                    cpal::platform::DeviceInner::Wasapi(a),
                    cpal::platform::DeviceInner::Wasapi(b),
                ) => a == b,
            });
        if !duplicate {
            devices.push((d, false));
        }
        if devices.len() > 16 {
            return Err(DeviceUnavailable);
        }
    }
    if devices.is_empty() {
        return Err(DeviceUnavailable);
    }
    devices
        .into_iter()
        .enumerate()
        .map(|(i, (d, default))| {
            let mut name: String = d
                .name()
                .map_err(|_| DeviceUnavailable)?
                .chars()
                .filter(|c| !c.is_control())
                .take(128)
                .collect();
            while name.len() > 128 {
                name.pop();
            }
            if name.is_empty() {
                name = format!("音频设备 {}", i + 1);
            }
            Ok((
                d,
                OutputDeviceView {
                    device_id: i as u32 + 1,
                    name,
                    is_default: default,
                },
            ))
        })
        .collect()
}
pub fn open(
    device: cpal::Device,
    pcm: zeroize::Zeroizing<Vec<i16>>,
    base: Instant,
    stop: Arc<AtomicBool>,
    deadline: Arc<AtomicU64>,
    progress: Arc<AtomicU64>,
) -> Result<cpal::Stream, DeviceUnavailable> {
    let halt = stop.clone();
    open_controlled(
        device,
        pcm,
        base,
        stop,
        Arc::new(move |now| {
            !halt.load(Ordering::Acquire) && now < deadline.load(Ordering::Acquire)
        }),
        progress,
    )
}
pub fn open_controlled(
    device: cpal::Device,
    pcm: zeroize::Zeroizing<Vec<i16>>,
    base: Instant,
    stop: Arc<AtomicBool>,
    allowed: Arc<dyn Fn(u64) -> bool + Send + Sync>,
    progress: Arc<AtomicU64>,
) -> Result<cpal::Stream, DeviceUnavailable> {
    let supported = device
        .default_output_config()
        .map_err(|_| DeviceUnavailable)?;
    let rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    if !(8000..=192000).contains(&rate) || channels == 0 || channels > 8 {
        return Err(DeviceUnavailable);
    }
    let mut cfg = supported.config();
    let mut frames = rate / 100;
    if let cpal::SupportedBufferSize::Range { min, max } = supported.buffer_size() {
        frames = frames.clamp(*min, *max);
    }
    if frames > rate / 10 {
        return Err(DeviceUnavailable);
    }
    cfg.buffer_size = cpal::BufferSize::Fixed(frames);
    let error_stop = stop.clone();
    let err = move |_| {
        error_stop.store(true, Ordering::Release);
    };
    let mut cursor = SubmissionCursor::new(rate, pcm.len()).ok_or(DeviceUnavailable)?;
    macro_rules! build {
        ($ty:ty,$convert:expr) => {{
            device
                .build_output_stream(
                    &cfg,
                    move |data: &mut [$ty], _| {
                        cursor::Render {
                            pcm: &pcm,
                            cursor: &mut cursor,
                            allowed: &|now| !stop.load(Ordering::Acquire) && allowed(now),
                            progress: &progress,
                            channels,
                        }
                        .fill(
                            data,
                            || base.elapsed().as_millis() as u64,
                            $convert,
                        );
                    },
                    err,
                    None,
                )
                .map_err(|_| DeviceUnavailable)?
        }};
    }
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build!(f32, |v: f32| v),
        cpal::SampleFormat::I16 => build!(i16, |v: f32| (v * 32767.0) as i16),
        cpal::SampleFormat::U16 => build!(u16, |v: f32| ((v + 1.0) * 32767.5) as u16),
        _ => return Err(DeviceUnavailable),
    };
    stream.play().map_err(|_| DeviceUnavailable)?;
    Ok(stream)
}

pub mod microphone;
