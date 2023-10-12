use std::path::PathBuf;
//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)
use iced::widget::{row, text};
use iced::{Alignment, Element, Sandbox, Settings};

pub fn main() -> iced::Result {
    Diph::run(Settings::default())
}

#[derive(Debug, Default)]
struct Diph {
    left_file: Option<PathBuf>,
    right_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
enum Message {
}

impl Sandbox for Diph {
    type Message = Message;

    fn new() -> Self {
        Default::default()
    }

    fn title(&self) -> String {
        "Diph".into()
    }

    fn update(&mut self, _message: Self::Message) {
    }

    fn view(&self) -> Element<Message> {
        row![
            text("hello").size(50),
        ]
            .padding(10)
            .align_items(Alignment::Center)
            .into()
    }
}