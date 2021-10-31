//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

// #![windows_subsystem = "windows"]
use clap::{App, Arg};
use druid::{AppLauncher, PlatformError, Vec2, WindowDesc};
use std::sync::{Arc, Mutex};

mod data;
mod delegate;
mod image_diff;
mod menu;
mod widgets;

fn main() -> Result<(), PlatformError> {
    let state = parse_cli()?;
    let window = WindowDesc::new(image_diff::build_ui(&state))
        .title("Diph")
        .menu(menu::build_menu);
    AppLauncher::with_window(window)
        .delegate(delegate::Delegate)
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
    let left = matches
        .value_of("LEFT")
        .map(String::from)
        .unwrap_or_else(|| "EMPTY".to_string());
    let right = matches
        .value_of("RIGHT")
        .map(String::from)
        .unwrap_or_else(|| "EMPTY".to_string());
    let state = data::AppState {
        left: data::ImagePreview::from_file(&left),
        right: data::ImagePreview::from_file(&right),
        zoom: Arc::new(Mutex::new(1.)),
        scroll_offset: Vec2::new(0., 0.),
    };
    Ok(state)
}
