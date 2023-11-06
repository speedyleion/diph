//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use iced::{executor, Application, Command, Element, Font, Settings, Theme};

mod widgets;
use crate::widgets::diff::{Diff, DiffMessage};

pub fn main() -> iced::Result {
    Diph::run(Settings {
        default_font: Font::MONOSPACE,
        ..Settings::default()
    })
}

struct Diph {
    diff: Diff,
}

#[derive(Debug, Clone)]
enum Message {
    Diff(DiffMessage),
}

impl Application for Diph {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        (Self { diff: Diff::new() }, Command::none())
    }

    fn title(&self) -> String {
        "Diph".into()
    }
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Diff(m) => self.diff.update(m),
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        self.diff.view().map(Message::Diff)
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
