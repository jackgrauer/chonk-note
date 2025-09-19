use helix_core::{Range, Rope, RopeSlice, Selection};

const TAB_WIDTH: usize = 4;

#[derive(Debug, Clone)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone)]
pub struct BlockSelection {
    // Anchor and cursor positions define the rectangle
    pub anchor: Position,  // (line, column)
    pub cursor: Position,  // (line, column)

    // Visual columns for handling tabs/wide chars
    pub anchor_visual_col: usize,
    pub cursor_visual_col: usize,
}

impl BlockSelection {
    pub fn new(anchor_line: usize, anchor_col: usize) -> Self {
        Self {
            anchor: Position::new(anchor_line, anchor_col),
            cursor: Position::new(anchor_line, anchor_col),
            anchor_visual_col: anchor_col,
            cursor_visual_col: anchor_col,
        }
    }

    pub fn extend_to(&mut self, line: usize, col: usize, visual_col: usize) {
        self.cursor.line = line;
        self.cursor.column = col;
        self.cursor_visual_col = visual_col;
    }

    pub fn normalized(&self) -> (Position, Position) {
        let min_line = self.anchor.line.min(self.cursor.line);
        let max_line = self.anchor.line.max(self.cursor.line);
        let min_col = self.anchor_visual_col.min(self.cursor_visual_col);
        let max_col = self.anchor_visual_col.max(self.cursor_visual_col);

        (Position::new(min_line, min_col), Position::new(max_line, max_col))
    }

    pub fn iter_lines(&self) -> impl Iterator<Item = (usize, usize, usize)> {
        let (start, end) = self.normalized();
        (start.line..=end.line).map(move |line| {
            (line, start.column, end.column)
        })
    }

    /// Convert block selection to multiple ranges (one per line)
    pub fn to_selection(&self, rope: &Rope) -> Selection {
        let mut ranges = Vec::new();
        let rope_slice = rope.slice(..);

        for (line_idx, start_col, end_col) in self.iter_lines() {
            if line_idx >= rope.len_lines() {
                break;
            }

            let line = rope_slice.line(line_idx);
            let line_start = rope.line_to_char(line_idx);

            // Convert visual columns to char indices
            let start_char = visual_col_to_char_idx(line, start_col);
            let end_char = visual_col_to_char_idx(line, end_col);

            // Clamp to line length
            let line_len = line.len_chars();
            let start_char = start_char.min(line_len);
            let end_char = end_char.min(line_len);

            let start = line_start + start_char;
            let end = line_start + end_char;

            if start <= end {
                ranges.push(Range::new(start, end));
            }
        }

        if ranges.is_empty() {
            // Fallback to a single point selection at cursor
            let pos = rope.line_to_char(self.cursor.line) + self.cursor.column;
            Selection::point(pos)
        } else {
            Selection::new(ranges.into(), 0)
        }
    }

    /// Get the visual boundaries for rendering
    pub fn visual_bounds(&self) -> ((usize, usize), (usize, usize)) {
        let min_line = self.anchor.line.min(self.cursor.line);
        let max_line = self.anchor.line.max(self.cursor.line);
        let min_col = self.anchor_visual_col.min(self.cursor_visual_col);
        let max_col = self.anchor_visual_col.max(self.cursor_visual_col);

        ((min_line, min_col), (max_line, max_col))
    }
}

pub fn visual_col_to_char_idx(line: RopeSlice, visual_col: usize) -> usize {
    let mut current_visual = 0;
    let mut char_idx = 0;

    for ch in line.chars() {
        if current_visual >= visual_col {
            break;
        }

        let width = match ch {
            '\t' => TAB_WIDTH - (current_visual % TAB_WIDTH),
            '\n' | '\r' => break,  // Stop at line ending
            _ => unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1),
        };

        current_visual += width;
        char_idx += 1;
    }

    char_idx
}

pub fn char_idx_to_visual_col(line: RopeSlice, char_idx: usize) -> usize {
    let mut current_visual = 0;
    let mut idx = 0;

    for ch in line.chars() {
        if idx >= char_idx {
            break;
        }

        let width = match ch {
            '\t' => TAB_WIDTH - (current_visual % TAB_WIDTH),
            '\n' | '\r' => break,
            _ => unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1),
        };

        current_visual += width;
        idx += 1;
    }

    current_visual
}

