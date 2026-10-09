use nai_engine::{USAGE, parse, run};
use std::process;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match parse(&args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("nai-engine: {message}\n{USAGE}");
            process::exit(2);
        }
    };
    if let Err(message) = run(config).await {
        eprintln!("nai-engine: {message}");
        process::exit(2);
    }
}
