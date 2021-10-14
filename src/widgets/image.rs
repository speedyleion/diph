//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use druid::piet::{InterpolationMode, PietImage};
use druid::widget::prelude::*;
use druid::{
    Affine, BoxConstraints, Data, Env, Event, EventCtx, ImageBuf, LayoutCtx, LifeCycle,
    LifeCycleCtx, PaintCtx, Size, UpdateCtx, Widget,
};

pub struct Image {
    // TODO make this image buf dynamic
    image_data: ImageBuf,
    paint_data: Option<PietImage>,
}

impl Image {
    /// An Image widget that mimics a druid::Image with the following limitations:
    ///
    /// - It's always InterpolationMode::NearestNeighbor
    /// - It's always FillStrat::None
    ///
    /// The widget has the following extra characteristics:
    ///
    /// - It will be have a checkerboard pattern behind the image area
    /// - The image data can be updated dynamically
    pub fn new(image_data: ImageBuf) -> Self {
        Image {
            image_data,
            paint_data: None,
        }
    }
}

impl<T: Data> Widget<T> for Image {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event, _data: &mut T, _env: &Env) {}

    fn lifecycle(&mut self, _ctx: &mut LifeCycleCtx, _event: &LifeCycle, _data: &T, _env: &Env) {}

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &T, _data: &T, _env: &Env) {}

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &T,
        _env: &Env,
    ) -> Size {
        bc.constrain(self.image_data.size())
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _data: &T, _env: &Env) {
        let image_size = self.image_data.size();

        if image_size.is_empty() {
            return;
        }

        let ctx_size = ctx.size();
        ctx.clip(ctx_size.to_rect());

        ctx.with_save(|ctx| {
            let piet_image = {
                let image_data = &self.image_data;
                self.paint_data
                    .get_or_insert_with(|| image_data.to_image(ctx.render_ctx))
            };

            let origin_x = (ctx_size.width - image_size.width) / 2.0;
            let origin_y = (ctx_size.height - image_size.height) / 2.0;
            let offset_matrix = Affine::new([1., 0., 0., 1., origin_x, origin_y]);

            ctx.transform(offset_matrix);
            ctx.draw_image(
                piet_image,
                image_size.to_rect(),
                InterpolationMode::NearestNeighbor,
            );
        });
    }
}
