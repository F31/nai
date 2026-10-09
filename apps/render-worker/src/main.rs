use nai_render_worker::Config;
// `run` and the stdout writer are named only by the Linux drill body below.
#[cfg(target_os = "linux")]
use nai_render_worker::run;
#[cfg(target_os = "linux")]
use std::io::Write;
use std::process;

/// In `--render-drill` mode stderr carries the inherited pool FD, so failures
/// must never print there. Events go to stdout; the exit code is non-zero.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let render_drill = args.first().map(String::as_str) == Some("--render-drill");
    let fail = |message: &str| -> ! {
        if !render_drill {
            eprintln!("{message}");
        }
        process::exit(2);
    };
    let config = match Config::parse(&args) {
        Ok(config) => config,
        Err(message) => fail(&message),
    };
    #[cfg(target_os = "linux")]
    {
        let pool = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/proc/self/fd/2")
        {
            Ok(file) => file,
            Err(_) => fail("frame pool FD unavailable"),
        };
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        if run(&config, &pool, &mut out).is_err() {
            process::exit(2);
        }
        let _ = out.flush();
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = config;
        fail("render drill requires the Linux tmpfs backend");
    }
}
