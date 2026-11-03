use std::path::PathBuf;

use anyhow::Context as _;
use clap::Parser;

/// A side-by-side diff tool.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The file shown on the left.
    left: PathBuf,
    /// The file shown on the right.
    right: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    for path in [&args.left, &args.right] {
        std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    }
    Ok(())
}
