//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use makepad_widgets::*;
live_design!{
    import makepad_draw::shader::std::*;
    import makepad_widgets::theme_desktop_dark::*;

    TextWindow = {{TextWindow}} {
        width: Fill,
        height: Fill,
        margin: 0,
        scroll_bars: <ScrollBars> {}
        draw_bg: {
            color: #f
        }
        gutter: {
            draw_depth: 1.0,
            text_style: <THEME_FONT_CODE> {},
            color: #f,
        }
    }

}

#[derive(Live)]
pub struct TextWindow {
    #[live]
    scroll_bars: ScrollBars,
    #[live]
    gutter: DrawText,
}

impl LiveHook for TextWindow {
    fn before_live_design(cx: &mut Cx) {
        register_widget!(cx, TextWindow)
    }
}

impl Widget for TextWindow {
    fn draw_walk_widget(&mut self, cx: &mut Cx2d, walk: Walk) -> WidgetDraw {
        todo!()
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.scroll_bars.redraw(cx);
    }
}

impl TextWindow {
}
