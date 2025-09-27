// Simplified Helix-native editor that works with minimal dependencies
use anyhow::Result;
use helix_core::{Range, Rope, Selection, Transaction, movement};
use std::collections::HashMap;

use crate::block_selection::BlockSelection;

/// Selection mode including our block selection
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionMode {
    Normal,
    Block,
    BlockInsert,
    Insert,
}

/// A simple document wrapper
pub struct Document {
    pub rope: Rope,
    pub selection: Selection,
}

impl Document {
    pub fn new(text: &str) -> Self {
        let rope = Rope::from(text);
        let selection = Selection::point(0);
        Self { rope, selection }
    }

    pub fn apply_transaction(&mut self, transaction: Transaction) -> Result<()> {
        // Apply the transaction to the rope
        transaction.apply(&mut self.rope);
        // Update selection based on the transaction
        self.selection = self.selection.clone().map(transaction.changes());
        Ok(())
    }
}

/// Commands for the editor
#[derive(Debug, Clone)]
pub enum Command {
    // Movement
    MoveUp(usize),
    MoveDown(usize),
    MoveLeft(usize),
    MoveRight(usize),
    MoveWordForward,
    MoveWordBackward,
    MoveLineStart,
    MoveLineEnd,

    // Selection
    ExtendUp(usize),
    ExtendDown(usize),
    ExtendLeft(usize),
    ExtendRight(usize),

    // Block selection
    StartBlockSelection,
    ExtendBlockSelection,
    BlockInsertMode,
    ExitBlockMode,

    // Editing
    InsertChar(char),
    InsertText(String),
    Delete,
    Backspace,

    // Modes
    NormalMode,
    InsertMode,

    // Clipboard
    Copy,
    Cut,
    Paste,
}

/// Simple Helix-based editor with block selection
pub struct SimpleHelixEditor {
    pub document: Document,
    pub mode: SelectionMode,
    pub block_selection: Option<BlockSelection>,
    pub virtual_cursor_col: Option<usize>,
    pub clipboard: String,
}

impl SimpleHelixEditor {
    pub fn new() -> Self {
        Self {
            document: Document::new(""),
            mode: SelectionMode::Normal,
            block_selection: None,
            virtual_cursor_col: None,
            clipboard: String::new(),
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            document: Document::new(text),
            mode: SelectionMode::Normal,
            block_selection: None,
            virtual_cursor_col: None,
            clipboard: String::new(),
        }
    }

    pub fn execute_command(&mut self, cmd: Command) -> Result<()> {
        use Command::*;

        match cmd {
            // Movement commands
            MoveUp(n) => self.move_up(n),
            MoveDown(n) => self.move_down(n),
            MoveLeft(n) => self.move_left(n),
            MoveRight(n) => self.move_right(n),

            MoveWordForward => self.move_word_forward(),
            MoveWordBackward => self.move_word_backward(),
            MoveLineStart => self.move_line_start(),
            MoveLineEnd => self.move_line_end(),

            // Selection extension
            ExtendUp(n) => self.extend_up(n),
            ExtendDown(n) => self.extend_down(n),
            ExtendLeft(n) => self.extend_left(n),
            ExtendRight(n) => self.extend_right(n),

            // Block selection
            StartBlockSelection => self.start_block_selection(),
            ExtendBlockSelection => self.extend_block_selection(),
            BlockInsertMode => self.enter_block_insert_mode(),
            ExitBlockMode => self.exit_block_mode(),

            // Editing
            InsertChar(c) => self.insert_char(c),
            InsertText(s) => self.insert_text(&s),
            Delete => self.delete(),
            Backspace => self.backspace(),

            // Mode switching
            NormalMode => self.normal_mode(),
            InsertMode => self.insert_mode(),

            // Clipboard
            Copy => self.copy(),
            Cut => self.cut(),
            Paste => self.paste(),
        }
    }

    // Movement implementations
    fn move_up(&mut self, n: usize) -> Result<()> {
        match self.mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                if let Some(ref mut block) = self.block_selection {
                    block.cursor.line = block.cursor.line.saturating_sub(n);
                    if let Some(col) = self.virtual_cursor_col {
                        block.cursor_visual_col = col;
                    }
                }
            }
            _ => {
                let text = self.document.rope.slice(..);
                let selection = self.document.selection.clone();

                let new_selection = selection.transform(|range| {
                    let pos = range.cursor(text);
                    let line = text.char_to_line(pos);

                    if line > 0 {
                        let new_line = line.saturating_sub(n);
                        let new_line_start = text.line_to_char(new_line);

                        // Preserve column if possible
                        let line_start = text.line_to_char(line);
                        let col = pos - line_start;
                        let new_line_len = text.line(new_line).len_chars().saturating_sub(1);
                        let new_pos = new_line_start + col.min(new_line_len);

                        Range::point(new_pos)
                    } else {
                        range
                    }
                });

                self.document.selection = new_selection;
            }
        }
        Ok(())
    }

    fn move_down(&mut self, n: usize) -> Result<()> {
        match self.mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                if let Some(ref mut block) = self.block_selection {
                    let max_line = self.document.rope.len_lines().saturating_sub(1);
                    block.cursor.line = (block.cursor.line + n).min(max_line);
                    if let Some(col) = self.virtual_cursor_col {
                        block.cursor_visual_col = col;
                    }
                }
            }
            _ => {
                let text = self.document.rope.slice(..);
                let selection = self.document.selection.clone();

                let new_selection = selection.transform(|range| {
                    let pos = range.cursor(text);
                    let line = text.char_to_line(pos);
                    let max_line = text.len_lines().saturating_sub(1);

                    if line < max_line {
                        let new_line = (line + n).min(max_line);
                        let new_line_start = text.line_to_char(new_line);

                        // Preserve column if possible
                        let line_start = text.line_to_char(line);
                        let col = pos - line_start;
                        let new_line_len = text.line(new_line).len_chars().saturating_sub(1);
                        let new_pos = new_line_start + col.min(new_line_len);

                        Range::point(new_pos)
                    } else {
                        range
                    }
                });

                self.document.selection = new_selection;
            }
        }
        Ok(())
    }

    fn move_left(&mut self, n: usize) -> Result<()> {
        match self.mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                if let Some(ref mut block) = self.block_selection {
                    block.cursor_visual_col = block.cursor_visual_col.saturating_sub(n);
                    block.cursor.column = block.cursor_visual_col;
                    self.virtual_cursor_col = Some(block.cursor_visual_col);
                }
            }
            _ => {
                let text = self.document.rope.slice(..);
                let selection = self.document.selection.clone();

                let new_selection = selection.transform(|range| {
                    let pos = range.cursor(text);
                    let new_pos = pos.saturating_sub(n);
                    Range::point(new_pos)
                });

                self.document.selection = new_selection;
            }
        }
        Ok(())
    }

    fn move_right(&mut self, n: usize) -> Result<()> {
        match self.mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                if let Some(ref mut block) = self.block_selection {
                    block.cursor_visual_col += n;
                    block.cursor.column = block.cursor_visual_col;
                    self.virtual_cursor_col = Some(block.cursor_visual_col);
                }
            }
            _ => {
                let text = self.document.rope.slice(..);
                let selection = self.document.selection.clone();
                let max_pos = text.len_chars();

                let new_selection = selection.transform(|range| {
                    let pos = range.cursor(text);
                    let new_pos = (pos + n).min(max_pos);
                    Range::point(new_pos)
                });

                self.document.selection = new_selection;
            }
        }
        Ok(())
    }

    fn move_word_forward(&mut self) -> Result<()> {
        self.exit_block_mode()?; // Block mode doesn't support word movement

        let text = self.document.rope.slice(..);
        let selection = self.document.selection.clone();

        let new_selection = selection.transform(|range| {
            movement::move_next_word_start(text, range, 1)
        });

        self.document.selection = new_selection;
        Ok(())
    }

    fn move_word_backward(&mut self) -> Result<()> {
        self.exit_block_mode()?;

        let text = self.document.rope.slice(..);
        let selection = self.document.selection.clone();

        let new_selection = selection.transform(|range| {
            movement::move_prev_word_start(text, range, 1)
        });

        self.document.selection = new_selection;
        Ok(())
    }

    fn move_line_start(&mut self) -> Result<()> {
        let text = self.document.rope.slice(..);
        let selection = self.document.selection.clone();

        let new_selection = selection.transform(|range| {
            let pos = range.cursor(text);
            let line = text.char_to_line(pos);
            let line_start = text.line_to_char(line);
            Range::point(line_start)
        });

        self.document.selection = new_selection;

        if let Some(ref mut block) = self.block_selection {
            block.cursor.column = 0;
            block.cursor_visual_col = 0;
            self.virtual_cursor_col = Some(0);
        }

        Ok(())
    }

    fn move_line_end(&mut self) -> Result<()> {
        let text = self.document.rope.slice(..);
        let selection = self.document.selection.clone();

        let new_selection = selection.transform(|range| {
            let pos = range.cursor(text);
            let line = text.char_to_line(pos);
            let line_start = text.line_to_char(line);
            let line_len = text.line(line).len_chars().saturating_sub(1);
            Range::point(line_start + line_len)
        });

        self.document.selection = new_selection;

        if let Some(ref mut block) = self.block_selection {
            let line_len = text.line(block.cursor.line).len_chars().saturating_sub(1);
            block.cursor.column = line_len;
            block.cursor_visual_col = line_len;
            self.virtual_cursor_col = Some(line_len);
        }

        Ok(())
    }

    // Block selection operations
    fn start_block_selection(&mut self) -> Result<()> {
        let text = self.document.rope.slice(..);
        let pos = self.document.selection.primary().cursor(text);

        let line = text.char_to_line(pos);
        let line_start = text.line_to_char(line);
        let col = pos - line_start;

        self.block_selection = Some(BlockSelection::new(line, col));
        self.mode = SelectionMode::Block;
        self.virtual_cursor_col = Some(col);

        Ok(())
    }

    fn extend_block_selection(&mut self) -> Result<()> {
        if self.mode != SelectionMode::Block {
            self.start_block_selection()?;
        }
        Ok(())
    }

    fn enter_block_insert_mode(&mut self) -> Result<()> {
        if self.block_selection.is_some() {
            self.mode = SelectionMode::BlockInsert;
        }
        Ok(())
    }

    fn exit_block_mode(&mut self) -> Result<()> {
        self.mode = SelectionMode::Normal;
        self.block_selection = None;
        self.virtual_cursor_col = None;
        Ok(())
    }

    // Insert operations
    fn insert_char(&mut self, ch: char) -> Result<()> {
        match self.mode {
            SelectionMode::BlockInsert => {
                if let Some(ref block) = self.block_selection {
                    let mut changes = Vec::new();

                    for (line_idx, start_col, _) in block.iter_lines() {
                        if line_idx >= self.document.rope.len_lines() {
                            break;
                        }

                        let line_start = self.document.rope.line_to_char(line_idx);
                        let line = self.document.rope.line(line_idx);
                        let pos = line_start + start_col.min(line.len_chars());

                        changes.push((pos, pos, Some(ch.to_string().into())));
                    }

                    let transaction = Transaction::change(&self.document.rope, changes.into_iter());
                    self.document.apply_transaction(transaction)?;
                }
            }
            SelectionMode::Insert => {
                let selection = &self.document.selection;
                let text = ch.to_string();
                let transaction = Transaction::change(
                    &self.document.rope,
                    selection.ranges().iter().map(|range| {
                        let pos = range.cursor(self.document.rope.slice(..));
                        (pos, pos, Some(text.clone().into()))
                    }),
                );
                self.document.apply_transaction(transaction)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn insert_text(&mut self, text: &str) -> Result<()> {
        if self.mode == SelectionMode::Insert {
            let selection = &self.document.selection;
            let text_str = text.to_string();
            let transaction = Transaction::change(
                &self.document.rope,
                selection.ranges().iter().map(|range| {
                    let pos = range.cursor(self.document.rope.slice(..));
                    (pos, pos, Some(text_str.clone().into()))
                }),
            );
            self.document.apply_transaction(transaction)?;
        }
        Ok(())
    }

    fn delete(&mut self) -> Result<()> {
        let selection = &self.document.selection;
        let transaction = Transaction::change(
            &self.document.rope,
            selection.ranges().iter().map(|range| {
                (range.from(), range.to(), None)
            }),
        );
        self.document.apply_transaction(transaction)?;
        Ok(())
    }

    fn backspace(&mut self) -> Result<()> {
        // Move left then delete
        self.move_left(1)?;
        self.delete()
    }

    // Mode switching
    fn normal_mode(&mut self) -> Result<()> {
        self.mode = SelectionMode::Normal;
        Ok(())
    }

    fn insert_mode(&mut self) -> Result<()> {
        self.mode = SelectionMode::Insert;
        Ok(())
    }

    // Clipboard operations
    fn copy(&mut self) -> Result<()> {
        match self.mode {
            SelectionMode::Block | SelectionMode::BlockInsert => {
                if let Some(ref block) = self.block_selection {
                    let mut copied = String::new();

                    for (line_idx, start_col, end_col) in block.iter_lines() {
                        if line_idx >= self.document.rope.len_lines() {
                            break;
                        }

                        let line = self.document.rope.line(line_idx);
                        let line_str = line.to_string();

                        let start = start_col.min(line_str.len());
                        let end = end_col.min(line_str.len());

                        if start < end {
                            copied.push_str(&line_str[start..end]);
                        }
                        copied.push('\n');
                    }

                    self.clipboard = copied;
                }
            }
            _ => {
                let text = self.document.rope.slice(..);
                let range = self.document.selection.primary();
                let selected_text = text.slice(range.from()..range.to()).to_string();
                self.clipboard = selected_text;
            }
        }
        Ok(())
    }

    fn cut(&mut self) -> Result<()> {
        self.copy()?;
        self.delete()
    }

    fn paste(&mut self) -> Result<()> {
        let clipboard = self.clipboard.clone();
        self.insert_text(&clipboard)
    }

    // Extension methods
    fn extend_up(&mut self, n: usize) -> Result<()> {
        if self.mode == SelectionMode::Block {
            if let Some(ref mut block) = self.block_selection {
                block.cursor.line = block.cursor.line.saturating_sub(n);
            }
        } else {
            let text = self.document.rope.slice(..);
            let selection = self.document.selection.clone();

            let new_selection = selection.transform(|range| {
                let head = range.head;
                let line = text.char_to_line(head);

                if line > 0 {
                    let new_line = line.saturating_sub(n);
                    let new_head = text.line_to_char(new_line);
                    Range::new(range.anchor, new_head)
                } else {
                    range
                }
            });

            self.document.selection = new_selection;
        }
        Ok(())
    }

    fn extend_down(&mut self, n: usize) -> Result<()> {
        if self.mode == SelectionMode::Block {
            if let Some(ref mut block) = self.block_selection {
                let max_line = self.document.rope.len_lines().saturating_sub(1);
                block.cursor.line = (block.cursor.line + n).min(max_line);
            }
        } else {
            let text = self.document.rope.slice(..);
            let selection = self.document.selection.clone();
            let max_line = text.len_lines().saturating_sub(1);

            let new_selection = selection.transform(|range| {
                let head = range.head;
                let line = text.char_to_line(head);

                if line < max_line {
                    let new_line = (line + n).min(max_line);
                    let new_head = text.line_to_char(new_line);
                    Range::new(range.anchor, new_head)
                } else {
                    range
                }
            });

            self.document.selection = new_selection;
        }
        Ok(())
    }

    fn extend_left(&mut self, n: usize) -> Result<()> {
        if self.mode == SelectionMode::Block {
            if let Some(ref mut block) = self.block_selection {
                block.cursor_visual_col = block.cursor_visual_col.saturating_sub(n);
            }
        } else {
            let text = self.document.rope.slice(..);
            let selection = self.document.selection.clone();

            let new_selection = selection.transform(|range| {
                let new_head = range.head.saturating_sub(n);
                Range::new(range.anchor, new_head)
            });

            self.document.selection = new_selection;
        }
        Ok(())
    }

    fn extend_right(&mut self, n: usize) -> Result<()> {
        if self.mode == SelectionMode::Block {
            if let Some(ref mut block) = self.block_selection {
                block.cursor_visual_col += n;
            }
        } else {
            let text = self.document.rope.slice(..);
            let selection = self.document.selection.clone();
            let max_pos = text.len_chars();

            let new_selection = selection.transform(|range| {
                let new_head = (range.head + n).min(max_pos);
                Range::new(range.anchor, new_head)
            });

            self.document.selection = new_selection;
        }
        Ok(())
    }
}

// Simple keymap
pub type KeyMap = HashMap<(helix_view::input::KeyCode, helix_view::input::KeyModifiers), Command>;

pub fn create_simple_keymap() -> KeyMap {
    use helix_view::input::{KeyCode, KeyModifiers};

    let mut map = KeyMap::new();

    // Arrow keys (no acceleration!)
    map.insert((KeyCode::Up, KeyModifiers::NONE), Command::MoveUp(1));
    map.insert((KeyCode::Down, KeyModifiers::NONE), Command::MoveDown(1));
    map.insert((KeyCode::Left, KeyModifiers::NONE), Command::MoveLeft(1));
    map.insert((KeyCode::Right, KeyModifiers::NONE), Command::MoveRight(1));

    // Vim style
    map.insert((KeyCode::Char('h'), KeyModifiers::NONE), Command::MoveLeft(1));
    map.insert((KeyCode::Char('j'), KeyModifiers::NONE), Command::MoveDown(1));
    map.insert((KeyCode::Char('k'), KeyModifiers::NONE), Command::MoveUp(1));
    map.insert((KeyCode::Char('l'), KeyModifiers::NONE), Command::MoveRight(1));

    // Word movement
    map.insert((KeyCode::Char('w'), KeyModifiers::NONE), Command::MoveWordForward);
    map.insert((KeyCode::Char('b'), KeyModifiers::NONE), Command::MoveWordBackward);

    // Line movement
    map.insert((KeyCode::Char('0'), KeyModifiers::NONE), Command::MoveLineStart);
    map.insert((KeyCode::Char('$'), KeyModifiers::NONE), Command::MoveLineEnd);

    // Block selection
    map.insert((KeyCode::Char('v'), KeyModifiers::CONTROL), Command::StartBlockSelection);
    map.insert((KeyCode::Esc, KeyModifiers::NONE), Command::ExitBlockMode);

    // Modes
    map.insert((KeyCode::Char('i'), KeyModifiers::NONE), Command::InsertMode);

    // Editing
    map.insert((KeyCode::Char('x'), KeyModifiers::NONE), Command::Delete);
    map.insert((KeyCode::Backspace, KeyModifiers::NONE), Command::Backspace);

    // Clipboard
    map.insert((KeyCode::Char('y'), KeyModifiers::NONE), Command::Copy);
    map.insert((KeyCode::Char('d'), KeyModifiers::NONE), Command::Cut);
    map.insert((KeyCode::Char('p'), KeyModifiers::NONE), Command::Paste);

    map
}