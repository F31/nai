//! V5.6.1 dependency probe entrypoint.
use nai_foundation_sqlite::v561::V561DataRoot;
use nai_runtime_core::v561_probe::{Overall, probe};

fn main() {
    let mut data_root: Option<String> = None;
    let mut strict = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-root" => data_root = args.next(),
            "--strict" => strict = true,
            "--help" | "-h" => {
                println!("usage: nai-v561-probe --data-root <path> [--strict]");
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    let Some(data_root) = data_root else {
        eprintln!("usage: nai-v561-probe --data-root <path> [--strict]");
        std::process::exit(2);
    };
    let root = match V561DataRoot::open(&data_root) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("v561 probe: cannot open data root: {error}");
            std::process::exit(3);
        }
    };
    let config = match root.load_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("v561 probe: cannot read config: {error}");
            std::process::exit(3);
        }
    };
    let report = probe(root.root(), &config);
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}"))
    );
    if strict && report.overall != Overall::Ok {
        std::process::exit(1);
    }
}
