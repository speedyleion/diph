//          Copyright Nick G 2023.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)

use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct FileBuffer {
    pub path: Option<PathBuf>,
    pub contents: Option<String>,
}

impl<T: AsRef<Path>> From<T> for FileBuffer {
    fn from(path: T) -> Self {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).ok();
        Self {
            path: Some(path.to_owned()),
            contents,
        }
    }
}
