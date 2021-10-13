//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::data::AppState;
use druid::keyboard_types::Key;
use druid::{commands, AppDelegate, DelegateCtx, Env, Event, WindowId};

pub struct Delegate;
impl AppDelegate<AppState> for Delegate {
    fn event(
        &mut self,
        ctx: &mut DelegateCtx,
        _window_id: WindowId,
        event: Event,
        _data: &mut AppState,
        _env: &Env,
    ) -> Option<Event> {
        match &event {
            Event::KeyDown(k) if k.key == Key::Escape => {
                ctx.submit_command(commands::QUIT_APP);
            }
            _ => {}
        }
        Some(event)
    }
}
