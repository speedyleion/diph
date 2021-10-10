use druid::widget::Flex;
use druid::{AppLauncher, PlatformError, Widget, WindowDesc};

mod image;

fn build_ui() -> impl Widget<()> {
    let left_image = include_bytes!("./assets/left.png");
    let right_image = include_bytes!("./assets/right.png");
    Flex::row()
        .with_flex_child(image::image(left_image), 1.0)
        .with_flex_child(image::image(right_image), 1.0)
}

fn main() -> Result<(), PlatformError> {
    let window = WindowDesc::new(build_ui()).title("Literate Broccoli");
    AppLauncher::with_window(window).launch(())?;
    Ok(())
}
