// Rope-based text buffer using ropey (same as Helix editor)
use ropey::Rope;
use std::cmp;

#[derive(Debug, Clone)]
pub struct Selection {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

impl Selection {
    pub fn new(row: usize, col: usize) -> Self {
        Self {
            start_row: row,
            start_col: col,
            end_row: row,
            end_col: col,
        }
    }

    pub fn update(&mut self, row: usize, col: usize) {
        self.end_row = row;
        self.end_col = col;
    }

    pub fn normalized(&self) -> (usize, usize, usize, usize) {
        let (start_row, start_col, end_row, end_col) = (
            self.start_row,
            self.start_col,
            self.end_row,
            self.end_col,
        );

        if start_row < end_row || (start_row == end_row && start_col <= end_col) {
            (start_row, start_col, end_row, end_col)
        } else {
            (end_row, end_col, start_row, start_col)
        }
    }

    pub fn contains(&self, row: usize, col: usize) -> bool {
        let (start_row, start_col, end_row, end_col) = self.normalized();

        if row < start_row || row > end_row {
            return false;
        }

        if row == start_row && row == end_row {
            col >= start_col && col <= end_col
        } else if row == start_row {
            col >= start_col
        } else if row == end_row {
            col <= end_col
        } else {
            true
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextBuffer {
    rope: Rope,
    pub selection: Option<Selection>,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::from_str("\n"),
            selection: None,
        }
    }

    pub fn from_string(content: &str) -> Self {
        let rope = if content.is_empty() {
            Rope::from_str("\n")
        } else {
            Rope::from_str(content)
        };

        Self {
            rope,
            selection: None,
        }
    }

    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn get_line(&self, row: usize) -> Option<String> {
        if row < self.rope.len_lines() {
            Some(self.rope.line(row).to_string())
        } else {
            None
        }
    }

    pub fn get_line_length(&self, row: usize) -> usize {
        if row < self.rope.len_lines() {
            let line = self.rope.line(row);
            let len = line.len_chars();
            // Don't count the newline
            if len > 0 && line.char(len - 1) == '\n' {
                len - 1
            } else {
                len
            }
        } else {
            0
        }
    }

    pub fn insert_char(&mut self, row: usize, col: usize, ch: char) {
        let char_idx = self.row_col_to_char_idx(row, col);
        self.rope.insert_char(char_idx, ch);
    }

    pub fn delete_char(&mut self, row: usize, col: usize) {
        let char_idx = self.row_col_to_char_idx(row, col);
        if char_idx < self.rope.len_chars() {
            self.rope.remove(char_idx..char_idx + 1);
        }
    }

    pub fn backspace(&mut self, row: usize, col: usize) -> Option<(usize, usize)> {
        if col > 0 {
            let char_idx = self.row_col_to_char_idx(row, col);
            if char_idx > 0 {
                self.rope.remove(char_idx - 1..char_idx);
                return Some((row, col - 1));
            }
        } else if row > 0 {
            // Join with previous line
            let prev_len = self.get_line_length(row - 1);
            let char_idx = self.row_col_to_char_idx(row, 0);
            if char_idx > 0 {
                // Remove the newline character
                self.rope.remove(char_idx - 1..char_idx);
                return Some((row - 1, prev_len));
            }
        }
        None
    }

    pub fn insert_newline(&mut self, row: usize, col: usize) -> (usize, usize) {
        let char_idx = self.row_col_to_char_idx(row, col);
        self.rope.insert_char(char_idx, '\n');
        (row + 1, 0)
    }

    pub fn insert_text(&mut self, row: usize, col: usize, text: &str) -> (usize, usize) {
        let char_idx = self.row_col_to_char_idx(row, col);
        self.rope.insert(char_idx, text);

        // Calculate new position
        let new_char_idx = char_idx + text.chars().count();
        self.char_idx_to_row_col(new_char_idx)
    }

    pub fn delete_selection(&mut self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        let (start_row, start_col, end_row, end_col) = sel.normalized();

        let start_idx = self.row_col_to_char_idx(start_row, start_col);
        let end_idx = self.row_col_to_char_idx(end_row, end_col + 1);

        if start_idx < end_idx && end_idx <= self.rope.len_chars() {
            let deleted = self.rope.slice(start_idx..end_idx).to_string();
            self.rope.remove(start_idx..end_idx);
            self.selection = None;
            return Some(deleted);
        }

        None
    }

    pub fn get_selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        let (start_row, start_col, end_row, end_col) = sel.normalized();

        let start_idx = self.row_col_to_char_idx(start_row, start_col);
        let end_idx = self.row_col_to_char_idx(end_row, end_col + 1);

        if start_idx < end_idx && end_idx <= self.rope.len_chars() {
            Some(self.rope.slice(start_idx..end_idx).to_string())
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.rope = Rope::from_str("\n");
        self.selection = None;
    }

    pub fn start_selection(&mut self, row: usize, col: usize) {
        self.selection = Some(Selection::new(row, col));
    }

    pub fn update_selection(&mut self, row: usize, col: usize) {
        if let Some(ref mut sel) = self.selection {
            sel.update(row, col);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    // Helper: Convert (row, col) to char index
    fn row_col_to_char_idx(&self, row: usize, col: usize) -> usize {
        let row = cmp::min(row, self.rope.len_lines().saturating_sub(1));
        let line_start = self.rope.line_to_char(row);
        let line_len = self.get_line_length(row);
        let col = cmp::min(col, line_len);
        line_start + col
    }

    // Helper: Convert char index to (row, col)
    fn char_idx_to_row_col(&self, char_idx: usize) -> (usize, usize) {
        let char_idx = cmp::min(char_idx, self.rope.len_chars());
        let row = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(row);
        let col = char_idx - line_start;
        (row, col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_char() {
        let mut buf = TextBuffer::new();
        buf.insert_char(0, 0, 'H');
        buf.insert_char(0, 1, 'i');
        assert_eq!(buf.get_line(0), Some("Hi\n"));
    }

    #[test]
    fn test_backspace() {
        let mut buf = TextBuffer::from_string("Hello\nWorld");
        buf.backspace(1, 0);
        assert_eq!(buf.to_string(), "HelloWorld");
    }

    #[test]
    fn test_selection() {
        let mut buf = TextBuffer::from_string("Hello World");
        buf.start_selection(0, 0);
        buf.update_selection(0, 4);
        assert_eq!(buf.get_selected_text(), Some("Hello".to_string()));
    }
}
