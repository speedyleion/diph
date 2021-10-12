#![windows_subsystem = "windows"]
use clap::{App, Arg};
use druid::{AppLauncher, PlatformError, WindowDesc};

mod data;
mod image_box;
mod image_diff;
mod menu;

fn main() -> Result<(), PlatformError> {
    let state = parse_cli()?;
    let window = WindowDesc::new(image_diff::build_ui(&state))
        .title("Literate Broccoli")
        .menu(menu::build_menu);
    AppLauncher::with_window(window)
        .launch(state)
        .expect("Failed to launch.");
    Ok(())
}

fn parse_cli() -> Result<data::AppState, PlatformError> {
    let matches = App::new("diph")
        .about("diffs images")
        .arg(Arg::with_name("LEFT").help("Sets the left file to compare"))
        .arg(Arg::with_name("RIGHT").help("Sets the right file to compare"))
        .get_matches();
    let state = data::AppState {
        left: matches.value_of("LEFT").map(String::from),
        right: matches.value_of("RIGHT").map(String::from),
    };
    Ok(state)
}
