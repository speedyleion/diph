//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use druid::debug_state::DebugState;

use druid::widget::prelude::*;
use druid::{Affine, Data, Size, WidgetPod};
use std::ops::Mul;
use std::sync::{Arc, Mutex};

/// Handles zooming
pub struct Zoom<T> {
    zoom: Arc<Mutex<f64>>,
    child: WidgetPod<T, Box<dyn Widget<T>>>,
}

impl<T> Zoom<T> {
    pub fn new(zoom: Arc<Mutex<f64>>, child: impl Widget<T> + 'static) -> Zoom<T> {
        Zoom {
            zoom,
            child: WidgetPod::new(child).boxed(),
        }
    }
}

impl<T: Data> Widget<T> for Zoom<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        self.child.event(ctx, event, data, env)
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        self.child.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        self.child.update(ctx, data, env);
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        let size = self.child.layout(ctx, &bc.loosen(), data, env);
        size.mul(*self.zoom.lock().unwrap())
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        ctx.with_save(|ctx| {
            let zoom_matrix = Affine::scale(*self.zoom.lock().unwrap());
            ctx.transform(zoom_matrix);
            self.child.paint_always(ctx, data, env);
        });
    }

    fn debug_state(&self, data: &T) -> DebugState {
        DebugState {
            display_name: self.short_type_name().to_string(),
            children: vec![self.child.widget().debug_state(data)],
            ..Default::default()
        }
    }
}
