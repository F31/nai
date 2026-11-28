//! Isolated shared-frame verification diagnostic.
fn main() {
    if let Err(error) =
        nai_media_core::v561_shared_receiver::receive(std::env::args_os().skip(1), |_, _| Ok(()))
    {
        eprintln!("v561 diagnostic receiver: {error}");
        std::process::exit(1);
    }
}
