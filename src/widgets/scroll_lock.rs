//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use druid::widget::prelude::*;
use druid::widget::Scroll;

use crate::data::AppState;
use druid::debug_state::DebugState;
use druid::{Point, Rect};

pub struct ScrollLock<W> {
    scroll: Scroll<AppState, W>,
}

impl<W: Widget<AppState>> ScrollLock<W> {
    pub fn new(child: W) -> Self {
        ScrollLock {
            scroll: Scroll::new(child),
        }
    }
}

impl<W: Widget<AppState>> Widget<AppState> for ScrollLock<W> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppState, env: &Env) {
        self.scroll.event(ctx, event, data, env);
        let new_offset = self.scroll.offset();
        if ctx.is_handled() {
            data.scroll_offset = new_offset;
        }
        if data.scroll_offset != new_offset {
            ctx.request_update();
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &AppState, env: &Env) {
        self.scroll.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &AppState, data: &AppState, env: &Env) {
        let origin = data.scroll_offset;
        self.scroll.scroll_to(Rect::from_origin_size(
            Point::new(origin.x, origin.y),
            ctx.size(),
        ));
        self.scroll.update(ctx, old_data, data, env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &AppState,
        env: &Env,
    ) -> Size {
        self.scroll.layout(ctx, &bc.loosen(), data, env)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppState, env: &Env) {
        self.scroll.paint(ctx, data, env)
    }

    fn debug_state(&self, data: &AppState) -> DebugState {
        DebugState {
            display_name: self.short_type_name().to_string(),
            children: vec![self.scroll.debug_state(data)],
            ..Default::default()
        }
    }
}
