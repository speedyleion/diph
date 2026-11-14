use diph_core::Position;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionKind {
    Character,
    Line,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    pub(crate) kind: SelectionKind,
    pub(crate) side: usize,
    pub(crate) anchor: Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SelectionRange {
    pub(crate) start: Position,
    pub(crate) end: Position,
}

impl Selection {
    pub(crate) fn new(kind: SelectionKind, side: usize, anchor: Position) -> Self {
        Self { kind, side, anchor }
    }

    pub(crate) fn range(&self, active: Position) -> SelectionRange {
        let (start, end) = if (self.anchor.line, self.anchor.column) <= (active.line, active.column)
        {
            (self.anchor, active)
        } else {
            (active, self.anchor)
        };
        SelectionRange { start, end }
    }
}

#[cfg(test)]
mod tests {
    use diph_core::Position;

    use super::{Selection, SelectionKind, SelectionRange};

    #[yare::parameterized(
        forward = {
            Position { line: 1, column: 2 },
            Position { line: 3, column: 4 },
            Position { line: 1, column: 2 },
            Position { line: 3, column: 4 },
        },
        reversed = {
            Position { line: 3, column: 4 },
            Position { line: 1, column: 2 },
            Position { line: 1, column: 2 },
            Position { line: 3, column: 4 },
        },
    )]
    fn range_is_ordered_by_position(
        anchor: Position,
        active: Position,
        start: Position,
        end: Position,
    ) {
        let selection = Selection::new(SelectionKind::Character, 1, anchor);

        assert_eq!(selection.range(active), SelectionRange { start, end });
    }
}
