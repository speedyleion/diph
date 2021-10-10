use druid::{AppLauncher, WindowDesc, Widget, PlatformError};
use druid::widget::Label;

fn build_ui() -> impl Widget<()> {
    Label::new("Hello world")
}

fn main() -> Result<(), PlatformError> {
    let window = WindowDesc::new(build_ui()).title("Literate Broccoli");
    AppLauncher::with_window(window).launch(())?;
    Ok(())
}