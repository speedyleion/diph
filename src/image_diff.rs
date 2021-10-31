//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::{AppState, ImagePreview};

use crate::widgets::{Image, ScrollLock, Zoom, ZoomController};
use dify::diff::get_results;
use druid::image::io::Reader as ImageReader;
use druid::image::{imageops, DynamicImage, ImageBuffer, Pixel, RgbaImage};
use druid::piet::{ImageFormat, InterpolationMode};
use druid::widget::prelude::*;
use druid::widget::{
    Align, Click, ControllerHost, Flex, Label, Painter, Split, Svg, SvgData, WidgetExt,
};
use druid::{Env, ImageBuf, PaintCtx, Rect, Widget};
use std::cmp::max;
use std::str::FromStr;
use std::sync::Arc;

pub fn build_ui(state: &AppState) -> impl Widget<AppState> {
    let left = build_left_source_ui(&state.left, state);
    let right = build_right_source_ui(&state.right, state);
    Split::rows(Split::columns(left, right), build_diff_ui(state))
}

fn build_left_source_ui(image: &ImagePreview, state: &AppState) -> impl Widget<AppState> {
    let text = build_filename(image);
    let icon = include_str!("assets/right-arrow-button.svg");
    let svg = Svg::new(SvgData::from_str(icon).unwrap());
    let button = ControllerHost::new(
        svg,
        Click::new(|_evt, data: &mut AppState, _env| {
            data.right.update(data.left.data());
        }),
    );
    let title_bar = Flex::row().with_flex_child(text, 1.0).with_child(button);
    build_source_ui(image, state, title_bar)
}

fn build_right_source_ui(image: &ImagePreview, state: &AppState) -> impl Widget<AppState> {
    let text = build_filename(image);
    let icon = include_str!("assets/left-arrow-button.svg");
    let svg = Svg::new(SvgData::from_str(icon).unwrap());
    let button = ControllerHost::new(
        svg,
        Click::new(|_evt, data: &mut AppState, _env| {
            data.left.update(data.right.data());
        }),
    );
    let title_bar = Flex::row().with_child(button).with_flex_child(text, 1.0);
    build_source_ui(image, state, title_bar)
}

fn build_source_ui(
    image: &ImagePreview,
    state: &AppState,
    title_bar: impl Widget<AppState> + 'static,
) -> impl Widget<AppState> {
    Flex::column().with_child(title_bar).with_flex_child(
        ScrollLock::new(Zoom::new(Arc::clone(&state.zoom), image_widget(image)))
            .background(Painter::new(draw_background))
            .center()
            .controller(ZoomController::new(Arc::clone(&state.zoom))),
        1.0,
    )
}

fn build_filename(image: &ImagePreview) -> impl Widget<AppState> {
    let name = image.filename();
    let label = match name {
        Some(name) => Label::new(name),
        None => Label::new("(empty)"),
    };
    Align::centered(label)
}

fn build_diff_ui(state: &AppState) -> impl Widget<AppState> {
    let image_buf = get_diff_image(&state.left.filename(), &state.right.filename());
    ScrollLock::new(Zoom::new(Arc::clone(&state.zoom), Image::new(image_buf)))
        .background(Painter::new(draw_background))
        .center()
        .controller(ZoomController::new(Arc::clone(&state.zoom)))
}

fn get_image_from_file(name: &Option<&str>) -> RgbaImage {
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

fn get_diff_image(left: &Option<&str>, right: &Option<&str>) -> ImageBuf {
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

fn image_widget(image: &ImagePreview) -> impl Widget<AppState> {
    let image_buf = match image.data() {
        Some(data) => {
            let image_data: Vec<u8> = data.iter().map(|a| (*a).to_owned()).collect();
            ImageBuf::from_data(image_data.as_slice()).unwrap_or_else(|_| ImageBuf::empty())
        }
        None => ImageBuf::empty(),
    };

    Image::new(image_buf)
}

fn draw_background(ctx: &mut PaintCtx, _data: &AppState, _env: &Env) {
    let rect = ctx.size().to_rect();
    // 40% and 60%
    let dimension = 16;
    let pixels = [
        102, 102, 102, 102, 102, 102, 102, 102, 153, 153, 153, 153, 153, 153, 153, 153, 102, 102,
        102, 102, 102, 102, 102, 102, 153, 153, 153, 153, 153, 153, 153, 153, 102, 102, 102, 102,
        102, 102, 102, 102, 153, 153, 153, 153, 153, 153, 153, 153, 102, 102, 102, 102, 102, 102,
        102, 102, 153, 153, 153, 153, 153, 153, 153, 153, 102, 102, 102, 102, 102, 102, 102, 102,
        153, 153, 153, 153, 153, 153, 153, 153, 102, 102, 102, 102, 102, 102, 102, 102, 153, 153,
        153, 153, 153, 153, 153, 153, 102, 102, 102, 102, 102, 102, 102, 102, 153, 153, 153, 153,
        153, 153, 153, 153, 102, 102, 102, 102, 102, 102, 102, 102, 153, 153, 153, 153, 153, 153,
        153, 153, 153, 153, 153, 153, 153, 153, 153, 153, 102, 102, 102, 102, 102, 102, 102, 102,
        153, 153, 153, 153, 153, 153, 153, 153, 102, 102, 102, 102, 102, 102, 102, 102, 153, 153,
        153, 153, 153, 153, 153, 153, 102, 102, 102, 102, 102, 102, 102, 102, 153, 153, 153, 153,
        153, 153, 153, 153, 102, 102, 102, 102, 102, 102, 102, 102, 153, 153, 153, 153, 153, 153,
        153, 153, 102, 102, 102, 102, 102, 102, 102, 102, 153, 153, 153, 153, 153, 153, 153, 153,
        102, 102, 102, 102, 102, 102, 102, 102, 153, 153, 153, 153, 153, 153, 153, 153, 102, 102,
        102, 102, 102, 102, 102, 102, 153, 153, 153, 153, 153, 153, 153, 153, 102, 102, 102, 102,
        102, 102, 102, 102,
    ];
    let pattern = ctx
        .make_image(dimension, dimension, &pixels, ImageFormat::Grayscale)
        .unwrap();
    ctx.clip(&rect);

    let columns = (rect.width() as usize + (dimension - 1)) / dimension;
    let rows = (rect.height() as usize + (dimension - 1)) / dimension;
    for col in 0..columns {
        let dimension_f64 = dimension as f64;
        let x0 = col as f64 * dimension_f64;
        for row in 0..rows {
            let y0 = row as f64 * dimension_f64;
            let rect = Rect::from_origin_size((x0, y0), (dimension_f64, dimension_f64));
            ctx.draw_image(&pattern, rect, InterpolationMode::Bilinear);
        }
    }
}
