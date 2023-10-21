//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::Message;
use iced::highlighter;
use iced::highlighter::Highlighter;
use iced::widget::{column, horizontal_space, row, text, text_editor};
use iced::Element;
use iced::Length;
use std::ffi;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) enum Action {
    Edit(text_editor::Action),
}

pub struct FileBuffer {
    pub path: Option<PathBuf>,
    pub content: text_editor::Content,
}

impl<T: AsRef<Path>> From<T> for FileBuffer {
    fn from(path: T) -> Self {
        let path = path.as_ref();
        let content = match std::fs::read_to_string(path).ok() {
            Some(contents) => text_editor::Content::with(&contents),
            None => text_editor::Content::new(),
        };
        Self {
            path: Some(path.to_owned()),
            content,
        }
    }
}

impl FileBuffer {
    pub(crate) fn update(&mut self, message: Message) {
        match message {
            Message::FileBuffer(action) => match action {
                Action::Edit(action) => self.content.edit(action),
            },
        }
    }

    pub(crate) fn view(&self) -> Element<Message> {
        let extension = self
            .path
            .as_deref()
            .and_then(Path::extension)
            .and_then(ffi::OsStr::to_str)
            .unwrap_or("rs");

        let buffer = text_editor(&self.content)
            .on_edit(action_to_message)
            .highlight::<Highlighter>(
                highlighter::Settings {
                    theme: highlighter::Theme::SolarizedDark,
                    extension: extension.into(),
                },
                |highlight, _theme| highlight.to_format(),
            );

        column![buffer, self.status_line_view()].into()
    }

    fn status_line_view(&self) -> Element<Message> {
        let file_name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(ffi::OsStr::to_str)
            .unwrap_or("New file");
        let (line, column) = self.content.cursor_position();
        let position = format!("{}:{}", line + 1, column + 1);
        row![
            text(file_name),
            horizontal_space(Length::Fill),
            text(position),
        ]
        .into()
    }
}

fn action_to_message(action: text_editor::Action) -> Message {
    Message::FileBuffer(Action::Edit(action))
}
