//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use iced::widget::pane_grid::Axis;
use iced::widget::{container, pane_grid, scrollable, text};
use iced::{executor, Application, Command, Element, Font, Length, Settings, Theme};

mod file_buffer;
use crate::file_buffer::FileBuffer;

pub fn main() -> iced::Result {
    Diph::run(Settings {
        default_font: Font::MONOSPACE,
        ..Settings::default()
    })
}

#[derive(Debug)]
struct Diph {
    panes: pane_grid::State<FileBuffer>,
}

#[derive(Debug, Clone, Copy)]
enum Message {}

impl Application for Diph {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        let file_1 = FileBuffer::from("src/main.rs");
        let (mut panes, pane) = pane_grid::State::new(file_1);
        let file_2 = FileBuffer::from("src/file_buffer.rs");
        let (_, _) = panes
            .split(Axis::Vertical, &pane, file_2)
            .expect("failed to split");
        (Self { panes }, Command::none())
    }

    fn title(&self) -> String {
        "Diph".into()
    }
    fn update(&mut self, _message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let pane_grid =
            pane_grid::PaneGrid::new(&self.panes, |_pane, file_buffer, _is_maximized| {
                pane_grid::Content::new(file_view(file_buffer))
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

fn file_view(file_buffer: &FileBuffer) -> Element<Message> {
    let contents = file_buffer.contents.as_deref().unwrap_or("");

    container(
        scrollable(text::Text::new(contents))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
