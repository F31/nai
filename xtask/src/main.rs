//! Command line shell for the workspace harness. The checks live in the
//! `nai_xtask` library so the profile can test them without linking this binary.
use std::{env, path::Path};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is at workspace root");
    if let Err(error) = nai_xtask::execute(&env::args().skip(1).collect::<Vec<_>>(), root) {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}
