//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::AppState;
use crate::image;
use druid::widget::{CrossAxisAlignment, Flex, FlexParams, Label, SizedBox};
use druid::Widget;

pub fn build_ui() -> impl Widget<AppState> {
    let left_raw = include_bytes!("./assets/left.png");
    let right_raw = include_bytes!("./assets/right.png");
    let left = build_source_ui("./assets/left.png", left_raw);
    let right = build_source_ui("./assets/right.png", right_raw);
    let centered = FlexParams::new(1.0, CrossAxisAlignment::Center);
    Flex::column()
        .with_flex_child(
            Flex::row()
                .with_flex_child(left, centered)
                .with_flex_child(right, centered),
            centered,
        )
        .with_flex_child(build_diff_ui(), centered)
}

fn build_source_ui(name: &str, raw_data: &[u8]) -> impl Widget<AppState> {
    let text = Label::new(name);
    Flex::column()
        .with_child(text)
        .with_flex_child(SizedBox::new(image::image(raw_data)).expand(), 1.0)
}

fn build_diff_ui() -> impl Widget<AppState> {
    let raw_data = include_bytes!("./assets/diff.png");
    SizedBox::new(image::image(raw_data)).expand()
}
