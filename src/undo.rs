/// Undo/Redo system for text buffer
use crate::text_buffer::TextBuffer;

#[derive(Clone, Debug)]
struct UndoState {
    content: String,
    cursor_row: usize,
    cursor_col: usize,
}

pub struct UndoStack {
    undo_stack: Vec<UndoState>,
    redo_stack: Vec<UndoState>,
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

    /// Save current state before making a change
    pub fn push_state(&mut self, buffer: &TextBuffer, cursor_row: usize, cursor_col: usize) {
        let state = UndoState {
            content: buffer.to_string(),
            cursor_row,
            cursor_col,
        };

        self.undo_stack.push(state);

        // Clear redo stack on new edit
        self.redo_stack.clear();

        // Limit undo stack size
        if self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Undo the last change
    pub fn undo(&mut self, buffer: &mut TextBuffer, cursor_row: &mut usize, cursor_col: &mut usize) -> bool {
        if self.undo_stack.is_empty() {
            return false;
        }

        // Save current state to redo stack
        let current = UndoState {
            content: buffer.to_string(),
            cursor_row: *cursor_row,
            cursor_col: *cursor_col,
        };
        self.redo_stack.push(current);

        // Restore previous state
        if let Some(state) = self.undo_stack.pop() {
            *buffer = TextBuffer::from_string(&state.content);
            *cursor_row = state.cursor_row;
            *cursor_col = state.cursor_col;
            return true;
        }

        false
    }

    /// Redo the last undone change
    pub fn redo(&mut self, buffer: &mut TextBuffer, cursor_row: &mut usize, cursor_col: &mut usize) -> bool {
        if self.redo_stack.is_empty() {
            return false;
        }

        // Save current state to undo stack
        let current = UndoState {
            content: buffer.to_string(),
            cursor_row: *cursor_row,
            cursor_col: *cursor_col,
        };
        self.undo_stack.push(current);

        // Restore redone state
        if let Some(state) = self.redo_stack.pop() {
            *buffer = TextBuffer::from_string(&state.content);
            *cursor_row = state.cursor_row;
            *cursor_col = state.cursor_col;
            return true;
        }

        false
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
