//! V5.6.1 Qwen3-TTS real probe. Requires the local service; not a CI test.
use nai_llm_openai::qwen::{Config, probe};

fn main() {
    let mut config = Config::default();
    let mut text = "你好，这是本地 Qwen3-TTS 探测。".to_string();
    let mut repeat = 1usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--endpoint" => config.endpoint = args.next().unwrap_or_default(),
            "--model" => config.model = args.next().unwrap_or_default(),
            "--voice" => config.voice = args.next().unwrap_or_default(),
            "--language" => config.language = args.next().unwrap_or_default(),
            "--text" => text = args.next().unwrap_or_default(),
            "--repeat" => {
                repeat = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(1)
                    .clamp(1, 10)
            }
            "--help" | "-h" => {
                println!(
                    "usage: nai-v561-tts-probe [--endpoint URL] [--model ID] [--voice NAME] [--language LANG] [--text TEXT] [--repeat N]"
                );
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    let reports: Vec<_> = (0..repeat).map(|_| probe(&config, &text)).collect();
    println!("{}", serde_json::to_string_pretty(&reports).unwrap());
    if reports.iter().any(|report| report.error.is_some()) {
        std::process::exit(1);
    }
}
