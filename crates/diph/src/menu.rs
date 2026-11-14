//! The menu bar, whose View > Themes menu picks the color theme. On macOS
//! it is the system menu bar at the top of the screen, elsewhere a bar at
//! the top of the window.

#[cfg(not(target_os = "macos"))]
#[path = "menu/in_window.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "menu/macos.rs"]
mod platform;

/// An action selected from the application menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuAction {
    Save,
    Reload,
    Theme(String),
}

pub use platform::MenuBar;
