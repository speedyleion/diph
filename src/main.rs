#![windows_subsystem = "windows"]
use druid::{AppLauncher, PlatformError, WindowDesc};

mod data;
mod image;
mod image_diff;
mod menu;

fn main() -> Result<(), PlatformError> {
    let window = WindowDesc::new(image_diff::build_ui())
        .title("Literate Broccoli")
        .menu(menu::build_menu);
    let state = data::AppState {
        foo: String::from("What"),
    };
    AppLauncher::with_window(window)
        .launch(state)
        .expect("Failed to launch.");
    Ok(())
}
