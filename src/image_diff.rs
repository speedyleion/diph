//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::AppState;

use crate::widgets::{Image, Zoom};
use dify::diff::get_results;
use druid::image::io::Reader as ImageReader;
use druid::image::{imageops, DynamicImage, ImageBuffer, Pixel, RgbaImage};
use druid::piet::ImageFormat;
use druid::widget::{CrossAxisAlignment, Flex, FlexParams, Label, Split, WidgetExt};
use druid::{ImageBuf, Widget};
use std::cmp::max;
use std::sync::Arc;

pub fn build_ui(state: &AppState) -> impl Widget<AppState> {
    let left = build_source_ui(&state.left, state);
    let right = build_source_ui(&state.right, state);
    let _centered = FlexParams::new(1.0, CrossAxisAlignment::Center);
    Split::rows(
        Split::columns(left, right),
        build_diff_ui(&state.left, &state.right),
    )
}

fn build_source_ui(name: &Option<String>, state: &AppState) -> impl Widget<AppState> {
    let text = match name {
        Some(name) => Label::new(name.as_str()),
        None => Label::new("(empty)"),
    };
    Flex::column().with_child(text).with_flex_child(
        Zoom::new(Arc::clone(&state.zoom), image_from_file(name))
            .scroll()
            .center(),
        1.0,
    )
}

fn build_diff_ui(left: &Option<String>, right: &Option<String>) -> impl Widget<AppState> {
    let image_buf = get_diff_image(left, right);
    Image::new(image_buf).scroll().center()
}

fn get_image_from_file(name: &Option<String>) -> RgbaImage {
    // TODO get some tests here for the failure cases and pass back the message string to
    // replace the file name in the UI for the user
    let name = match name {
        None => return DynamicImage::new_rgba8(1, 1).to_rgba8(),
        Some(name) => name,
    };

    let image_file = match ImageReader::open(name) {
        Ok(image_file) => image_file,
        Err(_) => return DynamicImage::new_rgba8(1, 1).to_rgba8(),
    };

    match image_file.decode() {
        Ok(image) => image.to_rgba8(),
        Err(_) => DynamicImage::new_rgba8(1, 1).to_rgba8(),
    }
}

fn get_diff_image(left: &Option<String>, right: &Option<String>) -> ImageBuf {
    let left_image = get_image_from_file(left);
    let right_image = get_image_from_file(right);

    let (left_width, left_height) = left_image.dimensions();
    let (right_width, right_height) = right_image.dimensions();
    let mut combined =
        ImageBuffer::new(max(left_width, right_width), max(left_height, right_height));
    imageops::overlay(&mut combined, &left_image, 0, 0);
    imageops::overlay(&mut combined, &right_image, 0, 0);

    let (width, height) = combined.dimensions();
    let mut greyscale = ImageBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let p = combined.get_pixel(x, y).to_luma_alpha().to_rgba();
            greyscale.put_pixel(x, y, p);
        }
    }

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
    if let Some((_, image)) = result {
        imageops::overlay(&mut greyscale, &image, 0, 0);
    }
    let size = greyscale.dimensions();
    ImageBuf::from_raw(
        greyscale.to_vec(),
        ImageFormat::RgbaSeparate,
        size.0 as usize,
        size.1 as usize,
    )
}

fn image_from_file(name: &Option<String>) -> impl Widget<AppState> {
    let image_buf = match name {
        Some(name) => ImageBuf::from_file(name).unwrap_or_else(|_| ImageBuf::empty()),
        None => ImageBuf::empty(),
    };

    Image::new(image_buf)
}
