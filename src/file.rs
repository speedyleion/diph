//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use iced::widget::Component;
use iced::widget::{component, row, text};
use iced::Element;
use iced::Renderer;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Copy)]
pub struct File;

#[derive(Debug, Default)]
pub struct FileBuffer {
    _path: Option<PathBuf>,
    contents: Option<String>,
}

impl<Message> Component<Message, Renderer> for File {
    type State = FileBuffer;
    type Event = ();

    fn update(&mut self, _state: &mut Self::State, _event: Self::Event) -> Option<Message> {
        None
    }

    fn view(&self, state: &Self::State) -> Element<Self::Event, Renderer> {
        row![text::Text::new(format!("{:?}", state.contents)),].into()
    }
}
impl<'a, Message> From<File> for Element<'a, Message, Renderer>
where
    Message: 'a,
{
    fn from(file: File) -> Self {
        component(file)
    }
}
