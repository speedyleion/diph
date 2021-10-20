//          Copyright Nick G 2021.
// Distributed under the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE or copy at
//          https://www.boost.org/LICENSE_1_0.txt)
use druid::{Data, Lens};
use std::sync::{Arc, Mutex};

#[derive(Clone, Data, Default, Lens)]
pub struct AppState {
    pub left: Option<String>,
    pub right: Option<String>,
    pub zoom: Arc<Mutex<f64>>,
}
