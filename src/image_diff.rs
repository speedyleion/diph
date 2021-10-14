//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::AppState;

use crate::widgets::Image;
use dify::diff::get_results;
use druid::image::io::Reader as ImageReader;
use druid::image::{DynamicImage, RgbaImage};
use druid::piet::ImageFormat;
use druid::widget::{CrossAxisAlignment, Flex, FlexParams, Label, Split, WidgetExt};
use druid::{ImageBuf, Widget};

pub fn build_ui(state: &AppState) -> impl Widget<AppState> {
    let left = build_source_ui(&state.left);
    let right = build_source_ui(&state.right);
    let _centered = FlexParams::new(1.0, CrossAxisAlignment::Center);
    Split::rows(
        Split::columns(left, right),
        build_diff_ui(&state.left, &state.right),
    )
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

fn build_diff_ui(left: &Option<String>, right: &Option<String>) -> impl Widget<AppState> {
    let image_buf = get_diff_image(left, right);
    Image::new(image_buf).scroll().center()
}

fn get_image_from_file(name: &Option<String>) -> RgbaImage {
    match name {
        Some(name) => ImageReader::open(name)
            .unwrap()
            .decode()
            .unwrap()
            .to_rgba8(),
        None => DynamicImage::new_rgba8(1, 1).to_rgba8(),
    }
}
fn get_diff_image(left: &Option<String>, right: &Option<String>) -> ImageBuf {
    let left_image = get_image_from_file(left);
    let right_image = get_image_from_file(right);
    let base = None;
    let block_out = None;
    let result = get_results(
        left_image,
        right_image,
        0.0f32,
        false,
        None,
        &base,
        &block_out,
    );
    match result {
        None => ImageBuf::empty(),
        Some((_, image)) => {
            let size = image.dimensions();
            ImageBuf::from_raw(
                image.to_vec(),
                ImageFormat::RgbaSeparate,
                size.0 as usize,
                size.1 as usize,
            )
        }
    }
}

fn image_from_file(name: &Option<String>) -> impl Widget<AppState> {
    let image_buf = match name {
        Some(name) => ImageBuf::from_file(name).unwrap_or_else(|_| ImageBuf::empty()),
        None => ImageBuf::empty(),
    };

    Image::new(image_buf)
}
