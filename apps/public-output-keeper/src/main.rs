#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod caption;
#[cfg(windows)]
mod preload;
#[cfg(windows)]
mod windows;
fn main() {
    if std::env::args().skip(1).eq(["--verify-media"]) {
        let run = || -> Result<(), ()> {
            if std::env::var_os("NAI_WORKBENCH_TOKEN").is_some() {
                return Err(());
            }
            // Validate the inherited capability itself: a writable file/pipe is rejected.
            #[cfg(windows)]
            {
                use std::{io::Write, os::windows::io::AsHandle};
                let stdin = std::io::stdin();
                let mut handle =
                    std::fs::File::from(stdin.as_handle().try_clone_to_owned().map_err(|_| ())?);
                if handle.write_all(&[0]).is_ok() {
                    return Err(());
                }
            }
            let d = nai_contracts::prepared_descriptor_from_env(
                &std::env::var("NAI_PREPARED_DESCRIPTOR").map_err(|_| ())?,
            )
            .map_err(|_| ())?;
            let id = std::env::var("NAI_PREPARED_CONSUMER").map_err(|_| ())?;
            let until = std::time::Instant::now() + std::time::Duration::from_secs(4);
            let receipt = nai_media_core::public_pipe::consume(
                &mut std::io::stdin().lock(),
                &d,
                &id,
                &|| {
                    if std::time::Instant::now() < until {
                        Ok(())
                    } else {
                        Err(nai_contracts::workbench::WorkbenchError::DeadlineExceeded)
                    }
                },
            )
            .map_err(|_| ())?;
            nai_contracts::audition_wire::write(&mut std::io::stdout().lock(), &receipt)
                .map_err(|_| ())
        };
        if run().is_err() {
            std::process::exit(1);
        }
        return;
    }
    #[cfg(windows)]
    if windows::run().is_err() {
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("PUBLIC_OUTPUT_PLATFORM_UNAVAILABLE");
        std::process::exit(1);
    }
}
