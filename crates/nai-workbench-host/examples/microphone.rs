use nai_workbench_host::human_mic::MicOwner;
use std::path::Path;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(!args.is_empty());
    let root = tempfile::tempdir().unwrap();
    let mut mic = MicOwner::open_at(Path::new(&args[0]), root.path()).unwrap();
    assert!(!mic.status.active);
    assert_eq!(mic.status.captured_frames, 0);
    let list = |v: &[nai_contracts::workbench::MicDevice]| {
        v.iter()
            .map(|d| serde_json::json!({"id":d.device_id,"name":d.name,"default":d.is_default}))
            .collect::<Vec<_>>()
    };
    println!(
        "{}",
        serde_json::json!({"inputs":list(&mic.status.inputs),"outputs":list(&mic.status.outputs),"active":mic.status.active})
    );
    if (3..=5).contains(&args.len()) {
        let cycles = if args.len() == 5 {
            args[4].parse::<usize>().unwrap().clamp(1, 3)
        } else {
            1
        };
        for _ in 0..cycles {
            let instance = mic.status.instance_id.clone();
            let s = mic
                .start(
                    &instance,
                    args[1].parse().unwrap(),
                    args[2].parse().unwrap(),
                    true,
                )
                .unwrap();
            assert!(s.active);
            println!(
                "MIC_ROUTE_ACTIVE input={} output={} capture=local_only recording=false",
                s.input_id, s.output_id
            );
            let seconds = if args.len() >= 4 {
                args[3].parse::<u64>().unwrap().clamp(1, 60)
            } else {
                3
            };
            let mut max_peak = 0;
            let began = std::time::Instant::now();
            let until = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
            while std::time::Instant::now() < until {
                let s = match mic.poll() {
                    Ok(s) => s,
                    Err(code) => {
                        eprintln!(
                            "MIC_FAILED elapsed_ms={} code={} captured={} submitted={} dropped={} peak={}",
                            began.elapsed().as_millis(),
                            code,
                            mic.status.captured_frames,
                            mic.status.submitted_frames,
                            mic.status.dropped_frames,
                            max_peak
                        );
                        drop(mic);
                        std::process::exit(1);
                    }
                };
                assert!(s.active);
                max_peak = max_peak.max(s.peak_q16);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            let s = mic.halt().unwrap();
            assert!(!s.active);
            println!(
                "{}",
                serde_json::json!({"captured_frames":s.captured_frames,"submitted_frames":s.submitted_frames,"dropped_frames":s.dropped_frames,"underrun_frames":s.underrun_frames,"peak_q16":s.peak_q16,"observed_max_peak_q16":max_peak,"stopped":!s.active,"pcm_recorded":false,"obs_verified":false})
            );
        }
    }
}
