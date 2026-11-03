mod text_view;

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use clap::Parser;
use diph_core::SideBySide;
use eframe::egui;

use text_view::TextView;

/// A side-by-side diff tool.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The file shown on the left.
    left: PathBuf,
    /// The file shown on the right.
    right: PathBuf,
}

impl eframe::App for TextView {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

fn read(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let diff = SideBySide::new(&read(&args.left)?, &read(&args.right)?);
    let (left_label, right_label) = (args.left.display(), args.right.display());
    let title = format!("diph — {left_label} ↔ {right_label}");
    let view = TextView::new(diff, left_label.to_string(), right_label.to_string());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "diph",
        options,
        Box::new(|cc| {
            // The diff colors are picked for a dark background.
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            Ok(Box::new(view))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
