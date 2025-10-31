/// Simplified undo/redo stub - disabled for now
use crate::text_buffer::TextBuffer;

pub struct UndoStack {
    _max_size: usize,
}

impl UndoStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            _max_size: max_size,
        }
    }

    pub fn undo(&mut self, _buffer: &mut TextBuffer) -> bool {
        // Undo disabled in simplified version
        false
    }

    pub fn redo(&mut self, _buffer: &mut TextBuffer) -> bool {
        // Redo disabled in simplified version
        false
    }

    pub fn clear(&mut self) {
        // Nothing to clear
    }
}
