//! Private inherited-stdio control slice. No listening port or media output.
fn main() {
    if nai_workbench_engine::v561_task_control::run(std::env::args().skip(1)).is_err() {
        eprintln!("NAI_TASK_CONTROL_FAILED");
        std::process::exit(1);
    }
}
