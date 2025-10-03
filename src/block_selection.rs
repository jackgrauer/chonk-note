use helix_core::{Rope, RopeSlice, Selection};

// Compatibility struct for existing code
#[derive(Clone, Debug)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub struct BlockSelection {
    // Store as (line, column) pairs
    pub start: (usize, usize),
    pub end: (usize, usize),

    // Compatibility fields for existing code that uses these
    pub anchor_visual_col: usize,
    pub cursor_visual_col: usize,
    pub cursor: Position,  // For compatibility
}

impl BlockSelection {
    pub fn new(line: usize, col: usize) -> Self {
        Self {
            start: (line, col),
            end: (line, col),
            anchor_visual_col: col,
            cursor_visual_col: col,
            cursor: Position { line, column: col },
        }
    }

    pub fn extend_to(&mut self, line: usize, col: usize, visual_col: usize) {
        self.end = (line, col);
        self.cursor.line = line;
        self.cursor.column = col;
        self.cursor_visual_col = visual_col;
    }

    /// Iterator for compatibility
    pub fn iter_lines(&self) -> impl Iterator<Item = (usize, usize, usize)> {
        let ((min_line, min_col), (max_line, max_col)) = self.normalized();
        (min_line..=max_line).map(move |line| {
            (line, min_col, max_col)
        })
    }

    pub fn normalized(&self) -> ((usize, usize), (usize, usize)) {
        let (start_line, start_col) = self.start;
        let (end_line, end_col) = self.end;

        // For lines, always normalize
        let min_line = start_line.min(end_line);
        let max_line = start_line.max(end_line);

        // For columns, use min/max but that's correct - we want the rectangular region
        let min_col = start_col.min(end_col);
        let max_col = start_col.max(end_col);

        ((min_line, min_col), (max_line, max_col))
    }

    pub fn contains(&self, line: usize, col: usize) -> bool {
        let ((min_line, min_col), (max_line, max_col)) = self.normalized();
        line >= min_line && line <= max_line &&
        col >= min_col && col <= max_col
    }

    /// Get the visual boundaries for rendering (same as normalized for simplicity)
    pub fn visual_bounds(&self) -> ((usize, usize), (usize, usize)) {
        self.normalized()
    }

    /// Convert block selection to multiple ranges (one per line)
    pub fn to_selection(&self, rope: &Rope) -> Selection {
        let mut ranges = Vec::new();
        let ((min_line, min_col), (max_line, max_col)) = self.normalized();

        for line_idx in min_line..=max_line {
            if line_idx >= rope.len_lines() {
                break;
            }

            let line_start = rope.line_to_char(line_idx);
            let line = rope.line(line_idx);
            let line_len = line.len_chars();

            // Use column positions directly, clamping to line length
            let start_char = min_col.min(line_len);
            let end_char = max_col.min(line_len);

            let start = line_start + start_char;
            let end = line_start + end_char;

            if start <= end {
                ranges.push(helix_core::Range::new(start, end));
            }
        }

        if ranges.is_empty() {
            // Fallback to a single point selection at cursor
            let (line, col) = self.end;
            let pos = if line < rope.len_lines() {
                let line_start = rope.line_to_char(line);
                let line_len = rope.line(line).len_chars();
                line_start + col.min(line_len)
            } else {
                rope.len_chars()
            };
            Selection::point(pos)
        } else {
            Selection::new(ranges.into(), 0)
        }
    }
}

// Helper functions for compatibility
pub fn char_idx_to_visual_col(_line: RopeSlice, char_idx: usize) -> usize {
    // For simplicity, treat each character as 1 column
    char_idx
}

pub fn visual_col_to_char_idx(_line: RopeSlice, visual_col: usize) -> usize {
    // For simplicity, treat each character as 1 column
    visual_col
}