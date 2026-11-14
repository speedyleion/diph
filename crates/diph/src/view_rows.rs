//! The rows of the view: the diff's rows, with blank rows after some of them.

/// Blank rows the view shows after a row of the diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Gap {
    /// The row of the diff the gap follows.
    pub(crate) after: usize,
    pub(crate) rows: usize,
}

/// What a row of the view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewRow {
    /// A row of the diff.
    Diff(usize),
    /// A row of the gap after a row of the diff.
    Gap { after: usize },
}

impl ViewRow {
    /// The row of the diff shown, or the one the gap follows.
    pub(crate) fn diff_row(self) -> usize {
        match self {
            Self::Diff(row) | Self::Gap { after: row } => row,
        }
    }
}

pub(crate) struct ViewRows {
    diff_rows: usize,
    /// In the order of the rows they follow, at most one per row.
    gaps: Vec<Gap>,
}

impl ViewRows {
    pub(crate) fn new(diff_rows: usize, gaps: Vec<Gap>) -> Self {
        Self { diff_rows, gaps }
    }

    pub(crate) fn len(&self) -> usize {
        self.diff_rows + self.gaps.iter().map(|gap| gap.rows).sum::<usize>()
    }

    /// The row of the view that shows row `row` of the diff. Past the last
    /// row of the diff, counts on from the end of the view.
    pub(crate) fn view_row(&self, row: usize) -> usize {
        let gap_rows = self.gaps.iter().take_while(|gap| gap.after < row);
        row + gap_rows.map(|gap| gap.rows).sum::<usize>()
    }

    /// What row `row` of the view shows.
    pub(crate) fn at(&self, row: usize) -> ViewRow {
        let mut diff_row = 0;
        let mut rest = row;
        for gap in &self.gaps {
            let rows_before_gap = gap.after + 1 - diff_row;
            if rest < rows_before_gap {
                break;
            }
            rest -= rows_before_gap;
            if rest < gap.rows {
                return ViewRow::Gap { after: gap.after };
            }
            rest -= gap.rows;
            diff_row = gap.after + 1;
        }
        ViewRow::Diff(diff_row + rest)
    }
}

#[cfg(test)]
mod tests {
    use super::{Gap, ViewRow, ViewRows};

    /// Ten rows of the diff, with two blank rows after row 2 and three after
    /// row 5.
    fn with_gaps() -> ViewRows {
        let gaps = vec![Gap { after: 2, rows: 2 }, Gap { after: 5, rows: 3 }];
        ViewRows::new(10, gaps)
    }

    #[test]
    fn gaps_add_their_rows_to_the_view() {
        assert_eq!(with_gaps().len(), 15);
    }

    #[yare::parameterized(
        a_row_before_any_gap_keeps_its_place = { 2, 2 },
        a_row_after_a_gap_moves_down_by_its_rows = { 3, 5 },
        a_row_after_two_gaps_moves_down_by_both = { 6, 11 },
        the_last_row_moves_down_by_every_gap = { 9, 14 },
    )]
    fn view_row(diff_row: usize, view_row: usize) {
        assert_eq!(with_gaps().view_row(diff_row), view_row);
    }

    #[yare::parameterized(
        a_row_before_any_gap = { 2, ViewRow::Diff(2) },
        the_first_row_of_a_gap = { 3, ViewRow::Gap { after: 2 } },
        the_last_row_of_a_gap = { 4, ViewRow::Gap { after: 2 } },
        the_row_after_a_gap = { 5, ViewRow::Diff(3) },
        a_row_of_the_second_gap = { 10, ViewRow::Gap { after: 5 } },
        the_last_row = { 14, ViewRow::Diff(9) },
    )]
    fn at(view_row: usize, shows: ViewRow) {
        assert_eq!(with_gaps().at(view_row), shows);
    }
}
