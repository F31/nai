#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
fn main() {
    #[cfg(windows)]
    {
        let a = std::env::args_os().skip(1).collect::<Vec<_>>();
        if a.len() != 3
            || nai_workbench_engine::v561_live_control::run(
                std::path::Path::new(&a[0]),
                std::path::Path::new(&a[1]),
                std::path::Path::new(&a[2]),
            )
            .is_err()
        {
            std::process::exit(1);
        }
    }
    #[cfg(not(windows))]
    std::process::exit(1);
}
