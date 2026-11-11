//! The menu bar, whose View > Themes menu picks the color theme. On macOS
//! it is the system menu bar at the top of the screen, elsewhere a bar at
//! the top of the window.

#[cfg(not(target_os = "macos"))]
#[path = "menu/in_window.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "menu/macos.rs"]
mod platform;

pub use platform::MenuBar;
