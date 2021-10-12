//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::AppState;
use crate::image_box;
use druid::widget::{CrossAxisAlignment, Flex, FlexParams, Label, SizedBox, Split};
use druid::Widget;

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
        .with_flex_child(SizedBox::new(image_box::image(name)).expand(), 1.0)
}

fn build_diff_ui() -> impl Widget<AppState> {
    let raw_data = include_bytes!("./assets/diff.png");
    SizedBox::new(image_box::image_from_raw(raw_data)).expand()
}
