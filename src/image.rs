//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use druid::{
    widget::{Image, FillStrat},
    piet::{ImageBuf, InterpolationMode},
};
use druid::piet::ImageFormat as PixelFormat;
use image::{ImageDecoder, ImageFormat, GenericImageView};
use image::codecs::png::PngDecoder;
use image::io::Reader;

pub fn image(raw_data: &[u8] ) -> Image {

    let image_data = image::load_from_memory_with_format(raw_data, ImageFormat::Png).unwrap();
    let (width, height) = image_data.dimensions();
    let image_buf = ImageBuf::from_raw(image_data.as_bytes(), PixelFormat::RgbaSeparate, width as usize, height as usize);
    let image_widget = Image::new(image_buf)
        // set the fill strategy
        .fill_mode(FillStrat::Fill)
        // set the interpolation mode
        .interpolation_mode(InterpolationMode::Bilinear);

    image_widget
}