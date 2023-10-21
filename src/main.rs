//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use iced::widget::pane_grid;
use iced::widget::pane_grid::{Axis, Pane};
use iced::{executor, Application, Command, Element, Font, Length, Settings, Theme};

mod file_buffer;
use crate::file_buffer::FileBuffer;

pub fn main() -> iced::Result {
    Diph::run(Settings {
        default_font: Font::MONOSPACE,
        ..Settings::default()
    })
}

struct Diph {
    panes: pane_grid::State<FileBuffer>,
    //HACK the first pane
    first_pane: Pane,
}

#[derive(Debug, Clone)]
enum Message {
    FileBuffer(file_buffer::Action),
}

impl Application for Diph {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        let file_1 = FileBuffer::from("src/main.rs");
        let (mut panes, pane) = pane_grid::State::new(file_1);
        let file_2 = FileBuffer::from("src/main.rs");
        let (_, _) = panes
            .split(Axis::Vertical, &pane, file_2)
            .expect("failed to split");
        (
            Self {
                panes,
                first_pane: pane,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Diph".into()
    }
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::FileBuffer(_) => {
                let buffer = self.panes.get_mut(&self.first_pane).unwrap();
                buffer.update(message)
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        pane_grid::PaneGrid::new(&self.panes, |_pane, file_buffer, _is_maximized| {
            pane_grid::Content::new(file_buffer.view())
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
