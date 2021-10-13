//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::AppState;
use druid::widget::{
    CrossAxisAlignment, FillStrat, Flex, FlexParams, Image, Label, Split, WidgetExt,
};
use druid::{ImageBuf, Widget};

pub fn build_ui(state: &AppState) -> impl Widget<AppState> {
    let left = build_source_ui(&state.left);
    let right = build_source_ui(&state.right);
    let _centered = FlexParams::new(1.0, CrossAxisAlignment::Center);
    Split::rows(Split::columns(left, right), build_diff_ui())
}

fn build_source_ui(name: &Option<String>) -> impl Widget<AppState> {
    let text = match name {
        Some(name) => Label::new(name.as_str()),
        None => Label::new("(empty)"),
    };
    Flex::column()
        .with_child(text)
        .with_flex_child(image_from_file(name).scroll().center(), 1.0)
}

fn build_diff_ui() -> impl Widget<AppState> {
    let raw_data = include_bytes!("./assets/diff.png");
    image_from_raw(raw_data).scroll().center()
}

fn image_from_file(name: &Option<String>) -> impl Widget<AppState> {
    let image_buf = match name {
        Some(name) => ImageBuf::from_file(name).unwrap_or_else(|_| ImageBuf::empty()),
        None => ImageBuf::empty(),
    };

    Image::new(image_buf).fill_mode(FillStrat::None)
}

fn image_from_raw(raw: &[u8]) -> impl Widget<AppState> {
    let image_buf = ImageBuf::from_data(raw).unwrap();

    Image::new(image_buf).fill_mode(FillStrat::None)
}
