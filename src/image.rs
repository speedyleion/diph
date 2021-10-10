//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use druid::piet::ImageFormat as PixelFormat;
use druid::{
    piet::{ImageBuf, InterpolationMode},
    widget::{FillStrat, Image},
};
use image::{GenericImageView, ImageFormat};

pub fn image(raw_data: &[u8]) -> Image {
    let image_data = image::load_from_memory_with_format(raw_data, ImageFormat::Png).unwrap();
    let (width, height) = image_data.dimensions();
    let image_buf = ImageBuf::from_raw(
        image_data.as_bytes(),
        PixelFormat::RgbaSeparate,
        width as usize,
        height as usize,
    );

    Image::new(image_buf)
        // set the fill strategy
        .fill_mode(FillStrat::Fill)
        // set the interpolation mode
        .interpolation_mode(InterpolationMode::Bilinear)
}
