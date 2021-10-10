//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::AppState;
use druid::{commands, Env, FileDialogOptions, LocalizedString, Menu, MenuItem, SysMods, WindowId};

pub fn build_menu(_window: Option<WindowId>, _data: &AppState, _env: &Env) -> Menu<AppState> {
    // TODO the runebender version handles Mac, once/if mac is supported this
    // would be something to look into.

    Menu::empty().entry(file_menu())
}

fn file_menu() -> Menu<AppState> {
    Menu::new(LocalizedString::new("common-menu-file-menu"))
        .entry(
            MenuItem::new(LocalizedString::new("common-menu-file-open"))
                .on_activate(|ctx, _, _| {
                    ctx.submit_command(commands::SHOW_OPEN_PANEL.with(FileDialogOptions::new()))
                })
                .hotkey(SysMods::Cmd, "o"),
        )
        .separator()
        .entry(
            MenuItem::new(LocalizedString::new("common-menu-file-save-as"))
                .on_activate(|ctx, _, _| {
                    ctx.submit_command(commands::SHOW_SAVE_PANEL.with(FileDialogOptions::new()))
                })
                .hotkey(SysMods::CmdShift, "S"),
        )
}
