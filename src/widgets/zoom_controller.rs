//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use druid::widget::prelude::*;
use druid::widget::Controller;
use std::sync::{Arc, Mutex};

pub struct ZoomController {
    zoom: Arc<Mutex<f64>>,
}

impl ZoomController {
    pub fn new(zoom: Arc<Mutex<f64>>) -> Self {
        ZoomController { zoom }
    }
}

impl<T, W: Widget<T>> Controller<T, W> for ZoomController {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        if let Event::Wheel(mouse) = event {
            let mut zoom = mouse.wheel_delta.y;
            if zoom < 0. {
                zoom = 1. / -zoom;
            }
            let mut s_zoom = self.zoom.lock().unwrap();
            *s_zoom *= zoom;
            ctx.request_layout();
        };
        child.event(ctx, event, data, env)
    }
}
