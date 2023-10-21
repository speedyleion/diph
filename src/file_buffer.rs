//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use std::ffi;
use iced::widget::text_editor;
use std::path::{Path, PathBuf};
use iced::highlighter;
use iced::highlighter::Highlighter;
use iced::Element;
use crate::Message;

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

pub(crate) fn file_view(file_buffer: &FileBuffer) -> Element<Message> {
    text_editor(&file_buffer.content)
        .highlight::<Highlighter>(
            highlighter::Settings {
                theme: highlighter::Theme::SolarizedDark,
                extension: file_buffer
                    .path
                    .as_deref()
                    .and_then(Path::extension)
                    .and_then(ffi::OsStr::to_str)
                    .map(str::to_string)
                    .unwrap_or(String::from("rs")),
            },
            |highlight, _theme| highlight.to_format(),
        )
        .into()
}
