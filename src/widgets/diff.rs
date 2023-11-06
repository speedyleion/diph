//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use crate::widgets::file_buffer::{FileBuffer, FileBufferMessage};
/// A widget that displays a diff of two files.
use iced::Element;
use iced_aw::split::Axis;
use iced_aw::Split;

#[derive(Debug, Clone)]
pub enum DiffMessage {
    Resized(u16),
    FileBuffer(FileBufferMessage),
}

#[derive(Debug, Clone)]
pub struct Diff {
    left: FileBuffer,
    right: FileBuffer,
    split_position: Option<u16>,
}

impl Diff {
    pub fn new() -> Self {
        Self {
            left: FileBuffer::from("src/widgets/diff.rs"),
            right: FileBuffer::from("src/widgets/file_buffer.rs"),
            split_position: None,
        }
    }

    pub fn update(&mut self, message: DiffMessage) {
        match message {
            DiffMessage::Resized(pos) => {
                self.split_position = Some(pos);
            }
            DiffMessage::FileBuffer(msg) => {
                let _ = self.left.update(msg.clone());
                let _ = self.right.update(msg);
            }
        }
    }

    pub fn view(&self) -> Element<DiffMessage> {
        Split::new(
            self.left.view().map(DiffMessage::FileBuffer),
            self.right.view().map(DiffMessage::FileBuffer),
            self.split_position,
            Axis::Vertical,
            DiffMessage::Resized,
        )
        .into()
    }
}
