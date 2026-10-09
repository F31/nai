use crate::schema::node;
use std::path::Path;

pub fn check(root: &Path) -> Result<(), String> {
    node(root, &["--test", "tests/registry.test.mjs"])?;
    println!("admission registry check passed (V3.6 acceptance + D01-D14 mapping)");
    Ok(())
}
