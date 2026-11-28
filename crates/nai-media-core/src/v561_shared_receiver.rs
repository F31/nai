//! Isolated diagnostic receiver. Not authenticated, not a production output path.
use crate::v561_frame_scheduler::{Frame, OwnedVerifiedFrame};
use nai_contracts::{
    v561_media::{FrameRef, FrameRelease},
    v561_media_validation::{FrameLimits, validate_frame},
};
use prost::Message;
use std::{
    io::{self, Read, Write},
    process::{Child, Command, Stdio},
};

struct Helper(Child);

#[derive(Debug)]
pub struct StopRequested;

impl std::fmt::Display for StopRequested {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("diagnostic stop requested")
    }
}

impl std::error::Error for StopRequested {}
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn receive<F>(
    args: impl Iterator<Item = std::ffi::OsString>,
    mut accept: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(FrameRef, Vec<u8>) -> Result<(), Box<dyn std::error::Error>>,
{
    receive_verified(args, |verified| {
        let frame = verified.into_frame();
        accept(frame.metadata, frame.pixels)
    })
}

pub fn receive_verified<F>(
    mut args: impl Iterator<Item = std::ffi::OsString>,
    mut accept: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(OwnedVerifiedFrame) -> Result<(), Box<dyn std::error::Error>>,
{
    let helper = args.next().ok_or("missing helper")?;
    let name = args.next().ok_or("missing mapping name")?;
    let slot_bytes: u64 = args
        .next()
        .ok_or("missing slot size")?
        .to_string_lossy()
        .parse()?;
    let slots: u32 = args
        .next()
        .ok_or("missing slots")?
        .to_string_lossy()
        .parse()?;
    let width: u32 = args
        .next()
        .ok_or("missing width")?
        .to_string_lossy()
        .parse()?;
    let height: u32 = args
        .next()
        .ok_or("missing height")?
        .to_string_lossy()
        .parse()?;
    let rate: u32 = args
        .next()
        .ok_or("missing sample rate")?
        .to_string_lossy()
        .parse()?;
    let epoch: u64 = args
        .next()
        .ok_or("missing epoch")?
        .to_string_lossy()
        .parse()?;
    if args.next().is_some()
        || !std::path::Path::new(&helper).is_absolute()
        || slots == 0
        || slots > 4
        || epoch == 0
    {
        return Err("invalid arguments".into());
    }
    let pool_bytes = slot_bytes
        .checked_mul(u64::from(slots))
        .ok_or("pool overflow")?;
    if pool_bytes == 0 || pool_bytes > 268435456 {
        return Err("invalid pool size".into());
    }
    let mut helper = Helper(
        Command::new(helper)
            .arg(&name)
            .arg(pool_bytes.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?,
    );
    let mut helper_in = helper.0.stdin.take().ok_or("missing helper input")?;
    let mut helper_out = helper.0.stdout.take().ok_or("missing helper output")?;
    let limits = FrameLimits {
        slots,
        max_width: width,
        max_height: height,
        max_payload_bytes: slot_bytes,
        max_sample_rate: rate,
    };
    let mut generations = vec![0; slots as usize];
    let mut expected = 0u64;
    let mut pool_id: Option<String> = None;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut len = [0u8; 4];
        match input.read(&mut len[..1])? {
            0 => return Err("frame input disconnected without completion".into()),
            1 => {}
            _ => unreachable!(),
        }
        input.read_exact(&mut len[1..])?;
        let header = u32::from_le_bytes(len);
        if header == u32::MAX {
            return Err(StopRequested.into());
        }
        let size = header as usize;
        // A zero-length header carries the final acknowledged frame count.
        if size == 0 {
            let mut count = [0; 8];
            input.read_exact(&mut count)?;
            if expected == 0 || u64::from_le_bytes(count) != expected {
                return Err("invalid frame completion count".into());
            }
            return Ok(());
        }
        if size > 4096 {
            return Err("invalid control length".into());
        }
        let mut wire = vec![0; size];
        input.read_exact(&mut wire)?;
        let frame = FrameRef::decode(wire.as_slice())?;
        validate_frame(&frame, limits).map_err(|_| "invalid frame")?;
        let slot = frame.slot_index.ok_or("missing slot")? as usize;
        if frame.epoch != epoch
            || frame.sequence != Some(expected)
            || frame.payload_bytes != slot_bytes
            || frame.source_sample_rate != rate
            || frame.width != width
            || frame.height != height
            || pool_id.as_ref().is_some_and(|pool| pool != &frame.pool_id)
            || frame.content_sha256.len() != 32
            || frame.slot_generation <= generations[slot]
        {
            return Err("frame identity rejected".into());
        }
        let offset = (slot as u64)
            .checked_mul(slot_bytes)
            .ok_or("offset overflow")?;
        helper_in.write_all(&offset.to_le_bytes())?;
        helper_in.write_all(&slot_bytes.to_le_bytes())?;
        helper_in.flush()?;
        let mut pixels = vec![0; usize::try_from(slot_bytes)?];
        helper_out.read_exact(&mut pixels)?;
        let verified = OwnedVerifiedFrame::verify(
            Frame {
                metadata: frame,
                pixels,
            },
            limits,
        )
        .map_err(|_| "frame integrity rejected")?;
        let frame = verified.metadata();
        generations[slot] = frame.slot_generation;
        if pool_id.is_none() {
            pool_id = Some(frame.pool_id.clone());
        }
        expected = expected.checked_add(1).ok_or("sequence overflow")?;
        let release = FrameRelease {
            pool_id: frame.pool_id.clone(),
            epoch,
            sequence: frame.sequence,
            slot_index: frame.slot_index,
            slot_generation: frame.slot_generation,
        }
        .encode_to_vec();
        accept(verified)?;
        output.write_all(&(release.len() as u32).to_le_bytes())?;
        output.write_all(&release)?;
        output.flush()?;
    }
}
