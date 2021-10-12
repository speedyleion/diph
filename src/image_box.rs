//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::AppState;
use druid::{
    piet::{ImageBuf, InterpolationMode},
    widget::{FillStrat, Image},
    Widget,
};

pub fn image(name: &Option<String>) -> impl Widget<AppState> {
    let image_buf = match name {
        Some(name) => ImageBuf::from_file(name).unwrap(),
        None => ImageBuf::empty(),
    };

    Image::new(image_buf)
        // set the fill strategy
        .fill_mode(FillStrat::None)
        // set the interpolation mode
        .interpolation_mode(InterpolationMode::Bilinear)
}

pub fn image_from_raw(raw: &[u8]) -> impl Widget<AppState> {
    let image_buf = ImageBuf::from_data(raw).unwrap();

    Image::new(image_buf)
        // set the fill strategy
        .fill_mode(FillStrat::None)
        // set the interpolation mode
        .interpolation_mode(InterpolationMode::Bilinear)
}
