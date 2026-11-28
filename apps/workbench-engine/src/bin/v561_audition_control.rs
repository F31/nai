fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 || args[0] != "--config" || args[2] != "--audio-output" {
        std::process::exit(2);
    }
    if nai_workbench_engine::v561_audition_control::run(
        args[1].clone().into(),
        args[3].clone().into(),
    )
    .is_err()
    {
        eprintln!("NAI_AUDITION_CONTROL_FAILED");
        std::process::exit(1);
    }
}
