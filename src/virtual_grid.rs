use helix_core::Rope;
use std::collections::HashMap;

/// VirtualGrid provides grid-aware editing capabilities on top of a rope.
/// It allows cursor movement and editing in "virtual" spaces past line endings,
/// which is essential for block selection and rectangular operations.
pub struct VirtualGrid {
    // Use the existing helix rope
    pub rope: Rope,
    // Track virtual spaces for block operations
    pub virtual_cols: HashMap<usize, usize>, // line_number -> rightmost_virtual_column
}

impl VirtualGrid {
    pub fn new(rope: Rope) -> Self {
        Self {
            rope,
            virtual_cols: HashMap::new(),
        }
    }

    /// Get character at grid position, returning space for virtual positions
    pub fn get_char_at(&self, col: usize, line_num: usize) -> Option<char> {
        if line_num >= self.rope.len_lines() {
            return None;
        }

        let line_start = self.rope.line_to_char(line_num);
        let line_end = if line_num + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line_num + 1).saturating_sub(1) // Exclude newline
        } else {
            self.rope.len_chars()
        };

        let line_len = line_end.saturating_sub(line_start);

        if col < line_len {
            Some(self.rope.char(line_start + col))
        } else {
            // Virtual space past line end
            Some(' ')
        }
    }

    /// Ensure a line has at least min_length characters by padding with spaces
    pub fn ensure_line_length(&mut self, line_num: usize, min_length: usize) {
        if line_num >= self.rope.len_lines() {
            // Add new lines if necessary
            let lines_to_add = line_num - self.rope.len_lines() + 1;
            let newlines = "\n".repeat(lines_to_add);
            self.rope.insert(self.rope.len_chars(), &newlines);
        }

        let line_start = self.rope.line_to_char(line_num);
        let line_end = if line_num + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line_num + 1).saturating_sub(1)
        } else {
            self.rope.len_chars()
        };

        let current_len = line_end.saturating_sub(line_start);

        if min_length > current_len {
            let padding = " ".repeat(min_length - current_len);
            self.rope.insert(line_end, &padding);

            // Track that we've extended this line virtually
            self.virtual_cols.insert(line_num, min_length);
        }
    }

    /// Set character at grid position, padding with spaces as needed
    /// IMPORTANT: This replaces in-place without shifting surrounding text
    pub fn set_char_at(&mut self, col: usize, line_num: usize, ch: char) {
        // First ensure the line exists and is long enough
        self.ensure_line_length(line_num, col + 1);

        let line_start = self.rope.line_to_char(line_num);
        let char_pos = line_start + col;

        if char_pos >= self.rope.len_chars() {
            return; // Position is beyond rope
        }

        // Convert to string, modify, convert back
        // This is safe because we're only replacing single characters
        let mut text = self.rope.to_string();
        let mut chars: Vec<char> = text.chars().collect();

        if char_pos < chars.len() {
            chars[char_pos] = ch;
            self.rope = Rope::from(chars.into_iter().collect::<String>());
        }
    }

    /// Get the actual line length (excluding virtual spaces)
    pub fn get_line_length(&self, line_num: usize) -> usize {
        if line_num >= self.rope.len_lines() {
            return 0;
        }

        let line_start = self.rope.line_to_char(line_num);
        let line_end = if line_num + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line_num + 1).saturating_sub(1)
        } else {
            self.rope.len_chars()
        };

        line_end.saturating_sub(line_start)
    }

    /// Get the virtual line length (including tracked virtual spaces)
    pub fn get_virtual_line_length(&self, line_num: usize) -> usize {
        let actual_len = self.get_line_length(line_num);
        self.virtual_cols
            .get(&line_num)
            .map(|&virtual_len| virtual_len.max(actual_len))
            .unwrap_or(actual_len)
    }

    /// Clear virtual space tracking for a line
    pub fn clear_virtual_cols(&mut self, line_num: usize) {
        self.virtual_cols.remove(&line_num);
    }

    /// Clear all virtual space tracking
    pub fn clear_all_virtual_cols(&mut self) {
        self.virtual_cols.clear();
    }

    /// Get a rectangular region of text
    pub fn get_block(&self, start_col: usize, start_line: usize, end_col: usize, end_line: usize) -> Vec<String> {
        let mut result = Vec::new();

        for line_num in start_line..=end_line {
            let mut line_text = String::new();
            for col in start_col..=end_col {
                if let Some(ch) = self.get_char_at(col, line_num) {
                    line_text.push(ch);
                }
            }
            result.push(line_text);
        }

        result
    }

    /// Set a rectangular region of text
    pub fn set_block(&mut self, start_col: usize, start_line: usize, lines: &[String]) {
        for (i, line_text) in lines.iter().enumerate() {
            let line_num = start_line + i;
            for (j, ch) in line_text.chars().enumerate() {
                self.set_char_at(start_col + j, line_num, ch);
            }
        }
    }

    /// Delete a rectangular region (replace with spaces)
    pub fn delete_block(&mut self, start_col: usize, start_line: usize, end_col: usize, end_line: usize) {
        for line_num in start_line..=end_line {
            for col in start_col..=end_col {
                self.set_char_at(col, line_num, ' ');
            }
        }
    }

    /// Cut a block selection, replacing with spaces (non-collapsing)
    pub fn cut_block(&mut self, selection: &crate::block_selection::BlockSelection) -> Vec<String> {
        let ((start_line, start_col), (end_line, end_col)) = selection.normalized();
        let mut cut_data = Vec::new();

        for line_idx in start_line..=end_line {
            let mut extracted = String::new();

            // Extract characters from the line
            for col in start_col..=end_col {
                if let Some(ch) = self.get_char_at(col, line_idx) {
                    extracted.push(ch);
                } else {
                    extracted.push(' ');
                }
            }
            cut_data.push(extracted);

            // Replace with spaces (non-collapsing)
            for col in start_col..=end_col {
                self.set_char_at(col, line_idx, ' ');
            }
        }

        cut_data
    }

    /// Paste block data at the cursor position
    pub fn paste_block(&mut self, cursor_line: usize, cursor_col: usize, data: &[String]) {
        for (i, line_data) in data.iter().enumerate() {
            let target_line = cursor_line + i;
            for (j, ch) in line_data.chars().enumerate() {
                let target_col = cursor_col + j;
                self.set_char_at(target_col, target_line, ch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_spaces() {
        let rope = Rope::from("Hello\nWorld");
        let mut grid = VirtualGrid::new(rope);

        // Get char past line end should return space
        assert_eq!(grid.get_char_at(10, 0), Some(' '));

        // Set char past line end should pad with spaces
        grid.set_char_at(10, 0, 'X');
        assert_eq!(grid.get_char_at(10, 0), Some('X'));
        assert_eq!(grid.get_char_at(9, 0), Some(' ')); // Padded space
    }

    #[test]
    fn test_block_operations() {
        let rope = Rope::from("12345\nABCDE\nfghij");
        let mut grid = VirtualGrid::new(rope);

        // Get a block
        let block = grid.get_block(1, 0, 3, 2);
        assert_eq!(block, vec!["234", "BCD", "ghi"]);

        // Set a block
        grid.set_block(1, 0, &["XXX".to_string(), "YYY".to_string()]);
        assert_eq!(grid.get_char_at(1, 0), Some('X'));
        assert_eq!(grid.get_char_at(1, 1), Some('Y'));
    }
}