//! Line diffing and syntax highlighting for diph.

mod buffer;
mod highlight;
mod side_by_side;

pub use buffer::Buffer;
pub use highlight::{Highlighter, Span};
pub use side_by_side::{Line, Row, RowKind, SideBySide};
