use nai_media_core::frame_slots::{PoolSpec, consumer::FrameReader};
use nai_output_keeper::{run, run_frame_drill};
use std::{
    process,
    sync::{Arc, atomic::AtomicBool},
};

const USAGE: &str = "Usage: nai-output-keeper --watchdog-drill <session> <trusted-epoch-floor> [--frame-drill <pool-id> <producer-ref> <consumer-ref> <slots> <slot-bytes>]; real devices unavailable";

/// In `--frame-drill` mode stderr carries the inherited pool FD, so failures
/// must never print there. The pool file is reopened read-only; frame data never
/// crosses the stdin/stdout control channel.
fn open_frame_reader(session: &str, epoch: u64, fields: &[String]) -> Option<FrameReader> {
    let [pool_id, producer, consumer, slots, slot_bytes] = fields else {
        return None;
    };
    let spec = PoolSpec {
        pool_id: pool_id.clone(),
        session_id: session.into(),
        generation_epoch: epoch,
        producer_ref: producer.clone(),
        consumer_ref: consumer.clone(),
        slots: slots.parse().ok()?,
        slot_bytes: slot_bytes.parse().ok()?,
    };
    #[cfg(target_os = "linux")]
    {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open("/proc/self/fd/2")
            .ok()?;
        FrameReader::open(file, spec).ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = spec;
        None
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let frame_drill = args.get(3).map(String::as_str) == Some("--frame-drill");
    let fail = |message: &str| -> ! {
        if !frame_drill {
            eprintln!("{message}");
        }
        process::exit(2);
    };
    if args.first().map(String::as_str) != Some("--watchdog-drill")
        || (args.len() != 3
            && !(frame_drill && args.len() >= 9 && (args.len() - 9).is_multiple_of(2)))
    {
        fail(USAGE);
    }
    let Some(floor) = args.get(2).and_then(|value| value.parse::<u64>().ok()) else {
        fail("invalid trusted epoch floor");
    };
    let session = &args[1];
    let mut frame_epoch = floor.max(1);
    let mut broker = None;
    if frame_drill {
        for extra in args[9..].chunks_exact(2) {
            match extra[0].as_str() {
                "--frame-epoch" => {
                    frame_epoch = extra[1]
                        .parse()
                        .unwrap_or_else(|_| fail("frame epoch rejected"))
                }
                "--broker" => broker = Some(std::path::PathBuf::from(&extra[1])),
                _ => fail(USAGE),
            }
        }
        if frame_epoch <= floor {
            fail("frame epoch must be above floor");
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    let outcome = if frame_drill {
        let Some(reader) = open_frame_reader(session, frame_epoch, &args[4..9]) else {
            fail("frame pool FD unavailable");
        };
        if let Some(path) = broker
            && nai_media_core::frame_slots::broker::connect_trigger(&path).is_err()
        {
            fail("broker trigger rejected");
        }
        run_frame_drill(
            session,
            floor,
            reader,
            std::io::stdin(),
            std::io::stdout(),
            stop,
        )
    } else {
        run(session, floor, std::io::stdin(), std::io::stdout(), stop)
    };
    if outcome.is_err() {
        fail("output watchdog drill failed; real devices unavailable");
    }
}
