// Helix-native editor with block selection support
use anyhow::Result;
use helix_core::{Position, Range, Rope, Selection, Transaction};
use helix_view::{Document, DocumentId, Editor, View, ViewId};
use std::sync::Arc;
use std::collections::HashMap;

use crate::block_selection::BlockSelection;

/// Extended selection mode that includes block selection
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionMode {
    Normal,           // Helix's normal selection
    Block,            // Our custom block selection
    BlockInsert,      // Block selection with insert mode
}

/// Commands that can be executed on the editor
#[derive(Debug, Clone)]
pub enum EditorCommand {
    // Helix native movements
    MoveUp(usize),
    MoveDown(usize),
    MoveLeft(usize),
    MoveRight(usize),
    MoveWordForward,
    MoveWordBackward,
    MoveLineStart,
    MoveLineEnd,
    MoveFileStart,
    MoveFileEnd,

    // Selection commands
    ExtendUp(usize),
    ExtendDown(usize),
    ExtendLeft(usize),
    ExtendRight(usize),

    // Block selection commands
    StartBlockSelection,
    ExtendBlockSelection,
    BlockInsertMode,
    ExitBlockMode,

    // Editing commands
    InsertChar(char),
    InsertNewline,
    Delete,
    Backspace,

    // Clipboard
    Copy,
    Cut,
    Paste,

    // Undo/Redo
    Undo,
    Redo,

    // Mode switching
    NormalMode,
    InsertMode,

    // Custom commands for notes/PDF
    CreateNote,
    SearchNotes,
    NextPdfPage,
    PrevPdfPage,
    ExtractPdfText,
}

/// Helix-native editor with block selection support
pub struct HelixNativeEditor {
    // Helix components
    pub editor: Editor,
    pub doc_id: DocumentId,

    // Our extensions
    pub selection_mode: SelectionMode,
    pub block_selection: Option<BlockSelection>,

    // Virtual cursor for maintaining column position
    pub virtual_cursor_col: Option<usize>,

    // Command history for undo/redo
    pub command_history: Vec<EditorCommand>,
}

impl HelixNativeEditor {
    pub fn new() -> Result<Self> {
        // Create Helix editor
        let mut editor = Editor::new(
            helix_view::graphics::Rect::new(0, 0, 80, 24),
            Arc::new(helix_view::editor::Config::default()),
        );

        // Create initial document
        let doc_id = editor.new_file(helix_view::editor::Action::VerticalSplit)?;

        Ok(Self {
            editor,
            doc_id,
            selection_mode: SelectionMode::Normal,
            block_selection: None,
            virtual_cursor_col: None,
            command_history: Vec::new(),
        })
    }

    pub fn handle_command(&mut self, cmd: EditorCommand) -> Result<()> {
        use EditorCommand::*;

        // Get current document and view
        let (view, doc) = self.editor.current_ref();

        match cmd {
            // Movement commands
            MoveUp(n) => self.move_up(n)?,
            MoveDown(n) => self.move_down(n)?,
            MoveLeft(n) => self.move_left(n)?,
            MoveRight(n) => self.move_right(n)?,

            MoveWordForward => self.move_word_forward()?,
            MoveWordBackward => self.move_word_backward()?,
            MoveLineStart => self.move_line_start()?,
            MoveLineEnd => self.move_line_end()?,
            MoveFileStart => self.move_file_start()?,
            MoveFileEnd => self.move_file_end()?,

            // Selection extension
            ExtendUp(n) => self.extend_selection_up(n)?,
            ExtendDown(n) => self.extend_selection_down(n)?,
            ExtendLeft(n) => self.extend_selection_left(n)?,
            ExtendRight(n) => self.extend_selection_right(n)?,

            // Block selection
            StartBlockSelection => self.start_block_selection()?,
            ExtendBlockSelection => self.extend_block_selection()?,
            BlockInsertMode => self.enter_block_insert_mode()?,
            ExitBlockMode => self.exit_block_mode()?,

            // Editing
            InsertChar(c) => self.insert_char(c)?,
            InsertNewline => self.insert_newline()?,
            Delete => self.delete()?,
            Backspace => self.backspace()?,

            // Clipboard
            Copy => self.copy()?,
            Cut => self.cut()?,
            Paste => self.paste()?,

            // Undo/Redo
            Undo => self.undo()?,
            Redo => self.redo()?,

            // Mode switching
            NormalMode => self.normal_mode()?,
            InsertMode => self.insert_mode()?,

            _ => {} // Custom commands handled elsewhere
        }

        // Store command for history
        self.command_history.push(cmd);

        Ok(())
    }

    // Movement implementations that respect block selection
    fn move_up(&mut self, n: usize) -> Result<()> {
        match self.selection_mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                // Update block selection cursor
                if let Some(ref mut block) = self.block_selection {
                    let new_line = block.cursor.line.saturating_sub(n);
                    block.cursor.line = new_line;

                    // Maintain visual column
                    if let Some(col) = self.virtual_cursor_col {
                        block.cursor_visual_col = col;
                    }
                }
            }
            SelectionMode::Normal => {
                // Use Helix's movement
                let (view, doc) = self.editor.current_mut();
                let text = doc.text().slice(..);
                let selection = doc.selection(view.id).clone();

                let new_selection = selection.transform(|range| {
                    let pos = range.cursor(text);
                    let line = text.char_to_line(pos);
                    if line > 0 {
                        let new_line = (line as isize - n as isize).max(0) as usize;
                        let new_pos = text.line_to_char(new_line);
                        Range::point(new_pos)
                    } else {
                        range
                    }
                });

                doc.set_selection(view.id, new_selection);
            }
        }
        Ok(())
    }

    fn move_down(&mut self, n: usize) -> Result<()> {
        match self.selection_mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                // Update block selection cursor
                if let Some(ref mut block) = self.block_selection {
                    let (_, doc) = self.editor.current_ref();
                    let max_line = doc.text().len_lines().saturating_sub(1);
                    let new_line = (block.cursor.line + n).min(max_line);
                    block.cursor.line = new_line;

                    // Maintain visual column
                    if let Some(col) = self.virtual_cursor_col {
                        block.cursor_visual_col = col;
                    }
                }
            }
            SelectionMode::Normal => {
                // Use Helix's movement
                let (view, doc) = self.editor.current_mut();
                let text = doc.text().slice(..);
                let selection = doc.selection(view.id).clone();

                let new_selection = selection.transform(|range| {
                    let pos = range.cursor(text);
                    let line = text.char_to_line(pos);
                    let max_line = text.len_lines().saturating_sub(1);
                    if line < max_line {
                        let new_line = (line + n).min(max_line);
                        let new_pos = text.line_to_char(new_line);
                        Range::point(new_pos)
                    } else {
                        range
                    }
                });

                doc.set_selection(view.id, new_selection);
            }
        }
        Ok(())
    }

    fn move_left(&mut self, n: usize) -> Result<()> {
        match self.selection_mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                // Update block selection cursor
                if let Some(ref mut block) = self.block_selection {
                    block.cursor_visual_col = block.cursor_visual_col.saturating_sub(n);
                    block.cursor.column = block.cursor_visual_col; // Simplified for now
                    self.virtual_cursor_col = Some(block.cursor_visual_col);
                }
            }
            SelectionMode::Normal => {
                // Use Helix's movement
                let (view, doc) = self.editor.current_mut();
                let text = doc.text().slice(..);
                let selection = doc.selection(view.id).clone();

                let new_selection = selection.transform(|range| {
                    let pos = range.cursor(text);
                    let new_pos = pos.saturating_sub(n);
                    Range::point(new_pos)
                });

                doc.set_selection(view.id, new_selection);
            }
        }
        Ok(())
    }

    fn move_right(&mut self, n: usize) -> Result<()> {
        match self.selection_mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                // Update block selection cursor
                if let Some(ref mut block) = self.block_selection {
                    block.cursor_visual_col += n;
                    block.cursor.column = block.cursor_visual_col; // Simplified for now
                    self.virtual_cursor_col = Some(block.cursor_visual_col);
                }
            }
            SelectionMode::Normal => {
                // Use Helix's movement
                let (view, doc) = self.editor.current_mut();
                let text = doc.text().slice(..);
                let selection = doc.selection(view.id).clone();

                let new_selection = selection.transform(|range| {
                    let pos = range.cursor(text);
                    let max_pos = text.len_chars();
                    let new_pos = (pos + n).min(max_pos);
                    Range::point(new_pos)
                });

                doc.set_selection(view.id, new_selection);
            }
        }
        Ok(())
    }

    // Block selection operations
    fn start_block_selection(&mut self) -> Result<()> {
        let (view, doc) = self.editor.current_ref();
        let selection = doc.selection(view.id);
        let pos = selection.primary().cursor(doc.text().slice(..));

        let line = doc.text().char_to_line(pos);
        let line_start = doc.text().line_to_char(line);
        let col = pos - line_start;

        self.block_selection = Some(BlockSelection::new(line, col));
        self.selection_mode = SelectionMode::Block;
        self.virtual_cursor_col = Some(col);

        Ok(())
    }

    fn extend_block_selection(&mut self) -> Result<()> {
        if self.selection_mode != SelectionMode::Block {
            self.start_block_selection()?;
        }

        // Block selection extension is handled by movement commands
        // when in block mode

        Ok(())
    }

    fn enter_block_insert_mode(&mut self) -> Result<()> {
        if self.block_selection.is_some() {
            self.selection_mode = SelectionMode::BlockInsert;
        }
        Ok(())
    }

    fn exit_block_mode(&mut self) -> Result<()> {
        self.selection_mode = SelectionMode::Normal;
        self.block_selection = None;
        self.virtual_cursor_col = None;
        Ok(())
    }

    // Insert character - handles block insert mode
    fn insert_char(&mut self, ch: char) -> Result<()> {
        match self.selection_mode {
            SelectionMode::BlockInsert => {
                // Insert character on all lines in block selection
                if let Some(ref block) = self.block_selection {
                    let (view, doc) = self.editor.current_mut();
                    let text = doc.text();

                    // Build transaction for all lines
                    let mut changes = Vec::new();
                    for (line_idx, start_col, _) in block.iter_lines() {
                        if line_idx >= text.len_lines() {
                            break;
                        }

                        let line_start = text.line_to_char(line_idx);
                        let pos = line_start + start_col.min(text.line(line_idx).len_chars());
                        changes.push((pos, pos, Some(ch.to_string().into())));
                    }

                    let transaction = Transaction::change(text, changes.into_iter());
                    doc.apply(&transaction, view.id);
                }
            }
            _ => {
                // Normal character insertion
                let (view, doc) = self.editor.current_mut();
                let transaction = Transaction::insert(
                    doc.text(),
                    doc.selection(view.id),
                    ch.to_string().as_str(),
                );
                doc.apply(&transaction, view.id);
            }
        }
        Ok(())
    }

    // Copy with block selection support
    fn copy(&mut self) -> Result<()> {
        match self.selection_mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                if let Some(ref block) = self.block_selection {
                    let (_, doc) = self.editor.current_ref();
                    let text = doc.text();

                    // Collect text from each line in block
                    let mut copied_text = String::new();
                    for (line_idx, start_col, end_col) in block.iter_lines() {
                        if line_idx >= text.len_lines() {
                            break;
                        }

                        let line = text.line(line_idx);
                        let line_text = line.to_string();
                        let start = start_col.min(line_text.len());
                        let end = end_col.min(line_text.len());

                        if start < end {
                            copied_text.push_str(&line_text[start..end]);
                        }
                        copied_text.push('\n');
                    }

                    // Store in clipboard (simplified - you'd use actual clipboard)
                    println!("Block copied: {} lines", block.iter_lines().count());
                }
            }
            SelectionMode::Normal => {
                // Use Helix's copy
                let (view, doc) = self.editor.current_ref();
                let selection = doc.selection(view.id);
                let text = doc.text().slice(..);

                for range in selection.ranges() {
                    let content = text.slice(range.from()..range.to()).to_string();
                    println!("Copied: {}", content);
                }
            }
        }
        Ok(())
    }

    // Helper methods for Helix movements
    fn move_word_forward(&mut self) -> Result<()> {
        self.exit_block_mode()?; // Block mode doesn't support word movement yet

        let (view, doc) = self.editor.current_mut();
        let selection = doc.selection(view.id).clone();
        let text = doc.text().slice(..);

        use helix_core::movement;
        let new_selection = selection.transform(|range| {
            movement::move_next_word_start(text, range, 1)
        });

        doc.set_selection(view.id, new_selection);
        Ok(())
    }

    fn move_word_backward(&mut self) -> Result<()> {
        self.exit_block_mode()?;

        let (view, doc) = self.editor.current_mut();
        let selection = doc.selection(view.id).clone();
        let text = doc.text().slice(..);

        use helix_core::movement;
        let new_selection = selection.transform(|range| {
            movement::move_prev_word_start(text, range, 1)
        });

        doc.set_selection(view.id, new_selection);
        Ok(())
    }

    fn move_line_start(&mut self) -> Result<()> {
        let (view, doc) = self.editor.current_mut();
        let selection = doc.selection(view.id).clone();
        let text = doc.text().slice(..);

        let new_selection = selection.transform(|range| {
            let line = text.char_to_line(range.cursor(text));
            let pos = text.line_to_char(line);
            Range::point(pos)
        });

        doc.set_selection(view.id, new_selection);

        // Update block selection if active
        if let Some(ref mut block) = self.block_selection {
            block.cursor.column = 0;
            block.cursor_visual_col = 0;
            self.virtual_cursor_col = Some(0);
        }

        Ok(())
    }

    fn move_line_end(&mut self) -> Result<()> {
        let (view, doc) = self.editor.current_mut();
        let selection = doc.selection(view.id).clone();
        let text = doc.text().slice(..);

        let new_selection = selection.transform(|range| {
            let line = text.char_to_line(range.cursor(text));
            let line_start = text.line_to_char(line);
            let line_end = line_start + text.line(line).len_chars().saturating_sub(1);
            Range::point(line_end)
        });

        doc.set_selection(view.id, new_selection);

        // Update block selection if active
        if let Some(ref mut block) = self.block_selection {
            let line = text.line(block.cursor.line);
            let len = line.len_chars().saturating_sub(1);
            block.cursor.column = len;
            block.cursor_visual_col = len;
            self.virtual_cursor_col = Some(len);
        }

        Ok(())
    }

    fn move_file_start(&mut self) -> Result<()> {
        self.exit_block_mode()?;

        let (view, doc) = self.editor.current_mut();
        doc.set_selection(view.id, Selection::point(0));
        Ok(())
    }

    fn move_file_end(&mut self) -> Result<()> {
        self.exit_block_mode()?;

        let (view, doc) = self.editor.current_mut();
        let end = doc.text().len_chars();
        doc.set_selection(view.id, Selection::point(end));
        Ok(())
    }

    // Extension methods
    fn extend_selection_up(&mut self, n: usize) -> Result<()> {
        if self.selection_mode == SelectionMode::Block {
            // Extend block selection
            if let Some(ref mut block) = self.block_selection {
                let new_line = block.cursor.line.saturating_sub(n);
                block.extend_to(new_line, block.cursor.column, block.cursor_visual_col);
            }
        } else {
            // Extend normal selection
            let (view, doc) = self.editor.current_mut();
            let text = doc.text().slice(..);
            let selection = doc.selection(view.id).clone();

            let new_selection = selection.transform(|range| {
                let head = range.head;
                let line = text.char_to_line(head);
                if line > 0 {
                    let new_line = (line as isize - n as isize).max(0) as usize;
                    let new_head = text.line_to_char(new_line);
                    Range::new(range.anchor, new_head)
                } else {
                    range
                }
            });

            doc.set_selection(view.id, new_selection);
        }
        Ok(())
    }

    fn extend_selection_down(&mut self, n: usize) -> Result<()> {
        if self.selection_mode == SelectionMode::Block {
            // Extend block selection
            if let Some(ref mut block) = self.block_selection {
                let (_, doc) = self.editor.current_ref();
                let max_line = doc.text().len_lines().saturating_sub(1);
                let new_line = (block.cursor.line + n).min(max_line);
                block.extend_to(new_line, block.cursor.column, block.cursor_visual_col);
            }
        } else {
            // Similar to extend_up but downward
            // Implementation here...
        }
        Ok(())
    }

    fn extend_selection_left(&mut self, n: usize) -> Result<()> {
        if self.selection_mode == SelectionMode::Block {
            // Extend block selection
            if let Some(ref mut block) = self.block_selection {
                let new_col = block.cursor_visual_col.saturating_sub(n);
                block.extend_to(block.cursor.line, new_col, new_col);
            }
        } else {
            // Extend normal selection left
            // Implementation here...
        }
        Ok(())
    }

    fn extend_selection_right(&mut self, n: usize) -> Result<()> {
        if self.selection_mode == SelectionMode::Block {
            // Extend block selection
            if let Some(ref mut block) = self.block_selection {
                let new_col = block.cursor_visual_col + n;
                block.extend_to(block.cursor.line, new_col, new_col);
            }
        } else {
            // Extend normal selection right
            // Implementation here...
        }
        Ok(())
    }

    // Stub implementations for other commands
    fn insert_newline(&mut self) -> Result<()> { Ok(()) }
    fn delete(&mut self) -> Result<()> { Ok(()) }
    fn backspace(&mut self) -> Result<()> { Ok(()) }
    fn cut(&mut self) -> Result<()> { Ok(()) }
    fn paste(&mut self) -> Result<()> { Ok(()) }
    fn undo(&mut self) -> Result<()> { Ok(()) }
    fn redo(&mut self) -> Result<()> { Ok(()) }
    fn normal_mode(&mut self) -> Result<()> { Ok(()) }
    fn insert_mode(&mut self) -> Result<()> { Ok(()) }
}