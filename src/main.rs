//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)
use iced::widget::pane_grid::Axis;
use iced::widget::{container, pane_grid};
use iced::{Element, Length, Sandbox, Settings, Theme};
mod file;
use crate::file::File;

pub fn main() -> iced::Result {
    Diph::run(Settings::default())
}

#[derive(Debug)]
struct Diph {
    panes: pane_grid::State<File>,
}

#[derive(Debug, Clone, Copy)]
enum Message {}

impl Sandbox for Diph {
    type Message = Message;

    fn new() -> Self {
        let (mut panes, pane) = pane_grid::State::new(File);
        let (_, _) = panes
            .split(Axis::Vertical, &pane, File)
            .expect("failed to split");
        Self { panes }
    }

    fn title(&self) -> String {
        "Diph".into()
    }

    fn update(&mut self, _message: Self::Message) {}

    fn view(&self) -> Element<Message> {
        let pane_grid = pane_grid::PaneGrid::new(&self.panes, |_pane, file, _is_maximized| {
            pane_grid::Content::new(*file)
        })
        .width(Length::Fill)
        .height(Length::Fill);

        container(pane_grid)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
