//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)
use druid::im::vector::Vector;
use druid::{Data, Lens, Vec2};
use std::sync::{Arc, Mutex};

use std::fs;
use std::path::Path;

#[derive(Clone, Data, Default, Lens)]
pub struct ImagePreview {
    #[lens(name = "_filename")]
    filename: Option<String>,
    #[lens(name = "_data")]
    data: Option<Vector<u8>>,
}

impl ImagePreview {
    pub fn from_file<P: AsRef<Path>>(path: P) -> ImagePreview {
        // TODO should better handle unable to open image
        let contents = fs::read(path.as_ref()).ok();
        let data = contents.map(Vector::from);

        ImagePreview {
            filename: Some(path.as_ref().to_string_lossy().to_string()),
            data,
        }
    }

    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    pub fn data(&self) -> Option<&Vector<u8>> {
        self.data.as_ref()
    }

    pub fn update(&mut self, new_data: Option<&Vector<u8>>) {
        self.data = new_data.cloned();
        // TODO I don't like this here it should happen on save
        let data: Vec<u8> = self.data.as_ref().unwrap().into_iter().copied().collect();
        fs::write(self.filename.as_ref().unwrap(), &data).unwrap();
    }
}

#[derive(Clone, Data, Default, Lens)]
pub struct AppState {
    pub left: ImagePreview,
    pub right: ImagePreview,
    pub zoom: Arc<Mutex<f64>>,
    pub scroll_offset: Vec2,
}
