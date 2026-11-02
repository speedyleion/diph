use std::path::PathBuf;

use anyhow::Context as _;
use clap::Parser;

/// A side-by-side diff tool.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The original file.
    old: PathBuf,
    /// The changed file.
    new: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    for path in [&args.old, &args.new] {
        std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    }
    Ok(())
}
