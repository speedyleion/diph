//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

/// A widget that displays an editable file buffer.
use iced::widget::{column, horizontal_space, row, scrollable, text};
use iced::Length;
use iced::{Command, Element};
use std::ffi;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum FileBufferMessage {
    _One,
}

#[derive(Debug, Clone)]
pub struct FileBuffer {
    pub path: Option<PathBuf>,
    pub content: String,
}

impl<T: AsRef<Path>> From<T> for FileBuffer {
    fn from(path: T) -> Self {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).unwrap_or_default();
        Self {
            path: Some(path.to_owned()),
            content,
        }
    }
}

impl FileBuffer {
    pub fn update(&mut self, _message: FileBufferMessage) -> Command<FileBufferMessage> {
        Command::none()
    }

    pub fn view(&self) -> Element<FileBufferMessage> {
        let _extension = self
            .path
            .as_deref()
            .and_then(Path::extension)
            .and_then(ffi::OsStr::to_str)
            .unwrap_or("rs");

        let buffer = text(&self.content);
        let scrolled_area = scrollable(buffer).width(Length::Fill).height(Length::Fill);

        column![scrolled_area, self.status_line_view()].into()
    }

    fn status_line_view(&self) -> Element<FileBufferMessage> {
        let file_name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(ffi::OsStr::to_str)
            .unwrap_or("New file");
        let (line, column) = (1, 1);
        let position = format!("{}:{}", line + 1, column + 1);
        row![
            text(file_name),
            horizontal_space(Length::Fill),
            text(position),
        ]
        .into()
    }
}
