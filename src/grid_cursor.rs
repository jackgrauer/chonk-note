use crate::virtual_grid::VirtualGrid;

/// GridCursor provides true grid-based cursor positioning
/// that can move anywhere in the virtual grid, not just where text exists
#[derive(Debug, Clone)]
pub struct GridCursor {
    pub row: usize,
    pub col: usize,
}

impl GridCursor {
    pub fn new() -> Self {
        Self { row: 0, col: 0 }
    }

    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
        }
    }

    pub fn move_down(&mut self, max_rows: usize) {
        if self.row < max_rows.saturating_sub(1) {
            self.row += 1;
        }
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        }
    }

    pub fn move_right(&mut self, _max_cols: usize) {
        // In grid mode, we allow moving as far right as needed
        self.col += 1;
    }

    pub fn move_to(&mut self, row: usize, col: usize) {
        self.row = row;
        self.col = col;
    }

    /// Convert grid position to rope char offset
    /// Returns None if position is in virtual space
    pub fn to_char_offset(&self, grid: &VirtualGrid) -> Option<usize> {
        if self.row >= grid.rope.len_lines() {
            return None;
        }

        let line_start = grid.rope.line_to_char(self.row);
        let line_end = if self.row + 1 < grid.rope.len_lines() {
            grid.rope.line_to_char(self.row + 1).saturating_sub(1)
        } else {
            grid.rope.len_chars()
        };

        let line_len = line_end.saturating_sub(line_start);

        if self.col < line_len {
            Some(line_start + self.col)
        } else {
            // Position is in virtual space
            None
        }
    }

    /// Create a GridCursor from a rope char offset
    pub fn from_char_offset(offset: usize, grid: &VirtualGrid) -> Self {
        let row = grid.rope.char_to_line(offset);
        let line_start = grid.rope.line_to_char(row);
        let col = offset.saturating_sub(line_start);

        Self { row, col }
    }
}