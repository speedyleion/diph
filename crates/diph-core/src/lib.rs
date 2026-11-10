//! Line diffing and syntax highlighting for diph.

mod buffer;
mod highlight;
mod side_by_side;
mod theme;

pub use buffer::{Buffer, BytePosition, Edit, Position};
pub use highlight::{Highlighter, SideColors, Span};
pub use side_by_side::{Line, Row, RowKind, SideBySide};
pub use theme::Theme;
