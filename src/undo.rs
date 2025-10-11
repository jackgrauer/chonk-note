/// Undo/Redo system using command pattern
use crate::chunked_grid::ChunkedGrid;

/// Unified command enum for all undo/redo operations
#[derive(Clone)]
pub enum Command {
    InsertChar {
        row: usize,
        col: usize,
        ch: char,
    },
    DeleteChar {
        row: usize,
        col: usize,
        deleted_char: char,
    },
    InsertNewLine {
        row: usize,
        col: usize,
        text_after_cursor: String,
    },
    DeleteLine {
        row: usize,
        deleted_line: String,
        prev_line_length: usize,
    },
    PasteBlock {
        row: usize,
        col: usize,
        lines: Vec<String>,
        replaced_content: Vec<String>,
    },
}

impl Command {
    pub fn execute(&self, grid: &mut ChunkedGrid) {
        match self {
            Command::InsertChar { row, col, ch } => {
                grid.shift_right(*row, *col, 1);
                grid.set(*row, *col, *ch);
            }
            Command::DeleteChar { row, col, .. } => {
                grid.delete_at(*row, *col);
                grid.shift_left(*row, *col, 1);
            }
            Command::InsertNewLine { row, col, text_after_cursor } => {
                execute_insert_newline(grid, *row, *col, text_after_cursor);
            }
            Command::DeleteLine { row, deleted_line, prev_line_length } => {
                execute_delete_line(grid, *row, deleted_line, *prev_line_length);
            }
            Command::PasteBlock { row, col, lines, .. } => {
                grid.paste_block(lines, *row, *col);
            }
        }
    }

    pub fn undo(&self, grid: &mut ChunkedGrid) {
        match self {
            Command::InsertChar { row, col, .. } => {
                grid.shift_left(*row, *col, 1);
            }
            Command::DeleteChar { row, col, deleted_char } => {
                grid.shift_right(*row, *col, 1);
                grid.set(*row, *col, *deleted_char);
            }
            Command::InsertNewLine { row, col, text_after_cursor } => {
                undo_insert_newline(grid, *row, *col, text_after_cursor);
            }
            Command::DeleteLine { row, deleted_line, prev_line_length } => {
                undo_delete_line(grid, *row, deleted_line, *prev_line_length);
            }
            Command::PasteBlock { row, col, lines, replaced_content } => {
                undo_paste_block(grid, *row, *col, lines, replaced_content);
            }
        }
    }
}

fn execute_insert_newline(grid: &mut ChunkedGrid, row: usize, col: usize, text_after_cursor: &str) {
    let lines = grid.to_lines();

    // Shift all lines below down by one (starting from bottom)
    for r in (row + 1..=lines.len()).rev() {
        let prev_line = if r > 0 && r - 1 < lines.len() {
            lines[r - 1].clone()
        } else {
            String::new()
        };

        // Clear the row
        for c in 0..1000 {
            grid.set(r, c, ' ');
        }

        // Write previous line content (unless it's the new line being created)
        if r != row + 1 {
            for (c, ch) in prev_line.chars().enumerate() {
                grid.set(r, c, ch);
            }
        }
    }

    // Clear text after cursor on current line
    for c in col..1000 {
        grid.set(row, c, ' ');
    }

    // Write text after cursor to next line
    for (c, ch) in text_after_cursor.chars().enumerate() {
        grid.set(row + 1, c, ch);
    }
}

fn undo_insert_newline(grid: &mut ChunkedGrid, row: usize, col: usize, text_after_cursor: &str) {
    let lines = grid.to_lines();

    // Clear the next line
    for c in 0..1000 {
        grid.set(row + 1, c, ' ');
    }

    // Restore text after cursor to current line
    for (i, ch) in text_after_cursor.chars().enumerate() {
        grid.set(row, col + i, ch);
    }

    // Shift all lines below up by one
    for r in (row + 1)..lines.len() {
        let next_line = if r + 1 < lines.len() {
            lines[r + 1].clone()
        } else {
            String::new()
        };

        // Clear the row
        for c in 0..1000 {
            grid.set(r, c, ' ');
        }

        // Write next line content
        for (c, ch) in next_line.chars().enumerate() {
            grid.set(r, c, ch);
        }
    }
}

fn execute_delete_line(grid: &mut ChunkedGrid, row: usize, deleted_line: &str, prev_line_length: usize) {
    let lines = grid.to_lines();

    // Append current line to previous line
    for (i, ch) in deleted_line.chars().enumerate() {
        grid.set(row - 1, prev_line_length + i, ch);
    }

    // Shift all lines below up by one
    for r in row..lines.len() {
        let next_line = if r + 1 < lines.len() {
            lines[r + 1].clone()
        } else {
            String::new()
        };

        // Clear the row
        for c in 0..1000 {
            grid.set(r, c, ' ');
        }

        // Write next line content
        for (c, ch) in next_line.chars().enumerate() {
            grid.set(r, c, ch);
        }
    }
}

fn undo_delete_line(grid: &mut ChunkedGrid, row: usize, deleted_line: &str, prev_line_length: usize) {
    let lines = grid.to_lines();

    // Shift all lines below down by one
    for r in (row..=lines.len()).rev() {
        let prev_line = if r > 0 && r - 1 < lines.len() {
            lines[r - 1].clone()
        } else {
            String::new()
        };

        // Clear the row
        for c in 0..1000 {
            grid.set(r, c, ' ');
        }

        // Write previous line content (unless it's the restored line)
        if r != row {
            for (c, ch) in prev_line.chars().enumerate() {
                grid.set(r, c, ch);
            }
        }
    }

    // Restore the deleted line
    for (c, ch) in deleted_line.chars().enumerate() {
        grid.set(row, c, ch);
    }

    // Remove the appended text from previous line
    for c in prev_line_length..1000 {
        grid.set(row - 1, c, ' ');
    }
}

fn undo_paste_block(grid: &mut ChunkedGrid, row: usize, col: usize, lines: &[String], replaced_content: &[String]) {
    // Clear the pasted content and restore original
    for (i, line) in lines.iter().enumerate() {
        let r = row + i;
        for c_idx in 0..line.len() {
            grid.set(r, col + c_idx, ' ');
        }
    }

    // Restore replaced content
    for (i, line) in replaced_content.iter().enumerate() {
        let r = row + i;
        for (c_idx, ch) in line.chars().enumerate() {
            grid.set(r, col + c_idx, ch);
        }
    }
}

/// Undo/Redo stack manager
pub struct UndoStack {
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,
    max_size: usize,
}

impl UndoStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }

    pub fn push(&mut self, command: Command) {
        // Clear redo stack when new command is added
        self.redo_stack.clear();

        self.undo_stack.push(command);

        // Limit stack size
        if self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self, grid: &mut ChunkedGrid) -> bool {
        if let Some(command) = self.undo_stack.pop() {
            command.undo(grid);
            self.redo_stack.push(command);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, grid: &mut ChunkedGrid) -> bool {
        if let Some(command) = self.redo_stack.pop() {
            command.execute(grid);
            self.undo_stack.push(command);
            true
        } else {
            false
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
