//! Explicit native microphone route. No disk, model, ASR or network path.
use super::DeviceUnavailable;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use nai_contracts::workbench::MicDevice;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};
use std::time::Instant;
use zeroize::Zeroize;

#[derive(Default)]
pub struct Counts {
    pub captured: AtomicU64,
    pub submitted: AtomicU64,
    pub dropped: AtomicU64,
    pub underrun: AtomicU64,
    pub peak: AtomicU32,
}
// One producer/consumer per CPAL FnMut callback. Atomic slots require no callback mutex.
struct Slot {
    sample: AtomicU32,
    stamp: AtomicU64,
}
struct Ring {
    data: Vec<Slot>,
    head: std::sync::atomic::AtomicUsize,
    tail: std::sync::atomic::AtomicUsize,
}
impl Ring {
    fn new(capacity: usize) -> Self {
        Self {
            data: (0..capacity)
                .map(|_| Slot {
                    sample: AtomicU32::new(0),
                    stamp: AtomicU64::new(0),
                })
                .collect(),
            head: 0.into(),
            tail: 0.into(),
        }
    }
    fn len(&self) -> usize {
        self.tail
            .load(Ordering::Acquire)
            .saturating_sub(self.head.load(Ordering::Acquire))
    }
    fn push(&self, value: f32, now: u64) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail - self.head.load(Ordering::Acquire) >= self.data.len() {
            return false;
        }
        let slot = &self.data[tail % self.data.len()];
        slot.sample.store(
            if value.is_finite() {
                value.clamp(-1., 1.)
            } else {
                0.
            }
            .to_bits(),
            Ordering::Relaxed,
        );
        slot.stamp.store(now, Ordering::Relaxed);
        self.tail.store(tail + 1, Ordering::Release);
        true
    }
    fn pop(&self, now: u64) -> (Option<f32>, u64) {
        let mut dropped = 0;
        loop {
            let head = self.head.load(Ordering::Relaxed);
            if head >= self.tail.load(Ordering::Acquire) {
                return (None, dropped);
            }
            let slot = &self.data[head % self.data.len()];
            let value = f32::from_bits(slot.sample.swap(0, Ordering::Relaxed));
            let stamp = slot.stamp.swap(0, Ordering::Relaxed);
            self.head.store(head + 1, Ordering::Release);
            if now < stamp || now - stamp > 200 {
                dropped += 1;
                continue;
            }
            return (Some(value), dropped);
        }
    }
}
impl Drop for Ring {
    fn drop(&mut self) {
        for slot in &mut self.data {
            slot.sample.get_mut().zeroize();
            slot.stamp.get_mut().zeroize();
        }
    }
}
pub fn devices(input: bool) -> Result<Vec<(cpal::Device, MicDevice)>, DeviceUnavailable> {
    let host = cpal::default_host();
    let mut all = Vec::new();
    let default = if input {
        host.default_input_device()
    } else {
        host.default_output_device()
    };
    if let Some(d) = default {
        all.push((d, true));
    }
    let rest = if input {
        host.input_devices()
    } else {
        host.output_devices()
    }
    .map_err(|_| DeviceUnavailable)?;
    for d in rest {
        if !all.iter().any(|(o, _)| match (o.as_inner(), d.as_inner()) {
            (cpal::platform::DeviceInner::Wasapi(a), cpal::platform::DeviceInner::Wasapi(b)) => {
                a == b
            }
        }) {
            all.push((d, false));
        }
        if all.len() >= 8 {
            break;
        }
    }
    all.into_iter()
        .enumerate()
        .map(|(i, (d, default))| {
            let mut name: String = d
                .name()
                .map_err(|_| DeviceUnavailable)?
                .chars()
                .filter(|c| !c.is_control())
                .take(96)
                .collect();
            while name.len() > 96 {
                name.pop();
            }
            if name.is_empty() {
                name = format!("设备 {}", i + 1)
            }
            Ok((
                d,
                MicDevice {
                    device_id: i as u32 + 1,
                    name,
                    is_default: default,
                },
            ))
        })
        .collect()
}
pub struct Relay {
    _input: cpal::Stream,
    _output: cpal::Stream,
}
#[allow(clippy::too_many_arguments)]
pub fn open(
    input: cpal::Device,
    output: cpal::Device,
    base: Instant,
    stop: Arc<AtomicBool>,
    deadline: Arc<AtomicU64>,
    counts: Arc<Counts>,
) -> Result<Relay, DeviceUnavailable> {
    let ic = input
        .default_input_config()
        .map_err(|_| DeviceUnavailable)?;
    let oc = output
        .default_output_config()
        .map_err(|_| DeviceUnavailable)?;
    let ir = ic.sample_rate().0;
    let or = oc.sample_rate().0;
    let inch = ic.channels() as usize;
    let outch = oc.channels() as usize;
    if !(8000..=192000).contains(&ir)
        || !(8000..=192000).contains(&or)
        || !(1..=8).contains(&inch)
        || !(1..=8).contains(&outch)
    {
        return Err(DeviceUnavailable);
    }
    let buffer = Arc::new(Ring::new(ir as usize / 5));
    let capture = |fmt| -> Result<cpal::Stream, DeviceUnavailable> {
        let buffer = buffer.clone();
        let stop = stop.clone();
        let deadline = deadline.clone();
        let counts = counts.clone();
        let error = stop.clone();
        macro_rules! make {
            ($ty:ty,$convert:expr) => {
                input
                    .build_input_stream(
                        &ic.config(),
                        move |data: &[$ty], _| {
                            let now = base.elapsed().as_millis() as u64;
                            if stop.load(Ordering::Acquire)
                                || now >= deadline.load(Ordering::Acquire)
                            {
                                return;
                            }
                            let mut peak = 0f32;
                            for frame in data.chunks_exact(inch) {
                                let sample =
                                    frame.iter().copied().map($convert).sum::<f32>() / inch as f32;
                                let sample = if sample.is_finite() {
                                    sample.clamp(-1., 1.)
                                } else {
                                    0.
                                };
                                peak = peak.max(sample.abs());
                                if !buffer.push(sample, now) {
                                    counts.dropped.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            counts
                                .captured
                                .fetch_add((data.len() / inch) as u64, Ordering::Release);
                            counts.peak.store((peak * 65535.) as u32, Ordering::Release);
                        },
                        move |_| {
                            error.store(true, Ordering::Release);
                        },
                        None,
                    )
                    .map_err(|_| DeviceUnavailable)
            };
        }
        match fmt {
            cpal::SampleFormat::F32 => make!(f32, |v: f32| v),
            cpal::SampleFormat::I16 => make!(i16, |v: i16| v as f32 / 32768.),
            cpal::SampleFormat::U16 => make!(u16, |v: u16| v as f32 / 32767.5 - 1.),
            _ => Err(DeviceUnavailable),
        }
    };
    let input_stream = capture(ic.sample_format())?;
    let error = stop.clone();
    macro_rules! render {
        ($ty:ty,$convert:expr) => {{
            let mut phase = 0u64;
            let mut last = None;
            output
                .build_output_stream(
                    &oc.config(),
                    move |data: &mut [$ty], _| {
                        let adjustment = if buffer.len() > buffer.data.len() * 3 / 4 {
                            1002u64
                        } else if buffer.len() < buffer.data.len() / 4 {
                            998
                        } else {
                            1000
                        };
                        let pop = |now| {
                            let (v, d) = buffer.pop(now);
                            counts.dropped.fetch_add(d, Ordering::Relaxed);
                            v
                        };
                        for frame in data.chunks_exact_mut(outch) {
                            let allowed = !stop.load(Ordering::Acquire)
                                && (base.elapsed().as_millis() as u64)
                                    < deadline.load(Ordering::Acquire);
                            if !allowed {
                                last = None;
                                phase = 0;
                                frame.fill($convert(0.));
                                continue;
                            }
                            if last.is_none() {
                                last = pop(base.elapsed().as_millis() as u64);
                            }
                            let value = last.unwrap_or(0.);
                            frame.fill($convert(value));
                            if last.is_some() {
                                counts.submitted.fetch_add(1, Ordering::Relaxed);
                            } else {
                                counts.underrun.fetch_add(1, Ordering::Relaxed);
                            }
                            phase += u64::from(ir) * adjustment;
                            while phase >= u64::from(or) * 1000 {
                                last = pop(base.elapsed().as_millis() as u64);
                                phase -= u64::from(or) * 1000;
                            }
                        }
                    },
                    move |_| {
                        error.store(true, Ordering::Release);
                    },
                    None,
                )
                .map_err(|_| DeviceUnavailable)?
        }};
    }
    let output_stream = match oc.sample_format() {
        cpal::SampleFormat::F32 => render!(f32, |v: f32| v),
        cpal::SampleFormat::I16 => render!(i16, |v: f32| (v * 32767.) as i16),
        cpal::SampleFormat::U16 => render!(u16, |v: f32| ((v + 1.) * 32767.5) as u16),
        _ => return Err(DeviceUnavailable),
    };
    output_stream.play().map_err(|_| DeviceUnavailable)?;
    input_stream.play().map_err(|_| DeviceUnavailable)?;
    Ok(Relay {
        _input: input_stream,
        _output: output_stream,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_ring_drops_full_and_stale_audio_and_zeroes_consumed_slots() {
        let r = Ring::new(2);
        assert!(r.push(0.2, 0));
        assert!(r.push(0.3, 0));
        assert!(!r.push(0.4, 0));
        assert_eq!(r.pop(1), (Some(0.2), 0));
        assert_eq!(r.data[0].sample.load(Ordering::Relaxed), 0);
        assert_eq!(r.pop(201), (None, 1));
        r.push(f32::NAN, 202);
        assert_eq!(r.pop(202), (Some(0.), 0));
    }
    #[test]
    fn atomic_ring_keeps_spsc_order_without_callback_locks() {
        let r = Arc::new(Ring::new(64));
        let p = r.clone();
        let t = std::thread::spawn(move || {
            for n in 0..10000 {
                while !p.push(n as f32 / 10000., 0) {
                    std::thread::yield_now();
                }
            }
        });
        for n in 0..10000 {
            loop {
                if let (Some(v), _) = r.pop(0) {
                    assert_eq!(v, n as f32 / 10000.);
                    break;
                }
                std::thread::yield_now();
            }
        }
        t.join().unwrap();
        assert_eq!(r.len(), 0);
    }
}
