// Keymap for Helix-native editor with block selection
use std::collections::HashMap;
use helix_view::input::{KeyCode, KeyEvent, KeyModifiers};

use crate::helix_native_editor::{EditorCommand, HelixNativeEditor, SelectionMode};

pub type KeyMap = HashMap<KeyEvent, EditorCommand>;

/// Create the default keymap with all our bindings
pub fn create_default_keymap() -> KeyMap {
    let mut keymap = KeyMap::new();

    // ==========================================
    // NORMAL MODE BINDINGS
    // ==========================================

    // Basic movement (no acceleration!)
    keymap.insert(key!(KeyCode::Up), EditorCommand::MoveUp(1));
    keymap.insert(key!(KeyCode::Down), EditorCommand::MoveDown(1));
    keymap.insert(key!(KeyCode::Left), EditorCommand::MoveLeft(1));
    keymap.insert(key!(KeyCode::Right), EditorCommand::MoveRight(1));

    // Vim-style movement
    keymap.insert(key!('h'), EditorCommand::MoveLeft(1));
    keymap.insert(key!('j'), EditorCommand::MoveDown(1));
    keymap.insert(key!('k'), EditorCommand::MoveUp(1));
    keymap.insert(key!('l'), EditorCommand::MoveRight(1));

    // Word movement
    keymap.insert(key!('w'), EditorCommand::MoveWordForward);
    keymap.insert(key!('b'), EditorCommand::MoveWordBackward);

    // macOS-style movement (Cmd + arrows)
    keymap.insert(
        key!(KeyCode::Left, KeyModifiers::SUPER),
        EditorCommand::MoveLineStart,
    );
    keymap.insert(
        key!(KeyCode::Right, KeyModifiers::SUPER),
        EditorCommand::MoveLineEnd,
    );
    keymap.insert(
        key!(KeyCode::Up, KeyModifiers::SUPER),
        EditorCommand::MoveFileStart,
    );
    keymap.insert(
        key!(KeyCode::Down, KeyModifiers::SUPER),
        EditorCommand::MoveFileEnd,
    );

    // Option + arrows for word movement (macOS style)
    keymap.insert(
        key!(KeyCode::Left, KeyModifiers::ALT),
        EditorCommand::MoveWordBackward,
    );
    keymap.insert(
        key!(KeyCode::Right, KeyModifiers::ALT),
        EditorCommand::MoveWordForward,
    );

    // Line movement
    keymap.insert(key!('0'), EditorCommand::MoveLineStart);
    keymap.insert(key!('$'), EditorCommand::MoveLineEnd);
    keymap.insert(key!(KeyCode::Home), EditorCommand::MoveLineStart);
    keymap.insert(key!(KeyCode::End), EditorCommand::MoveLineEnd);

    // File movement
    keymap.insert(key!('g', 'g'), EditorCommand::MoveFileStart);
    keymap.insert(key!('G'), EditorCommand::MoveFileEnd);

    // ==========================================
    // SELECTION EXTENSION (Shift + movement)
    // ==========================================

    keymap.insert(
        key!(KeyCode::Up, KeyModifiers::SHIFT),
        EditorCommand::ExtendUp(1),
    );
    keymap.insert(
        key!(KeyCode::Down, KeyModifiers::SHIFT),
        EditorCommand::ExtendDown(1),
    );
    keymap.insert(
        key!(KeyCode::Left, KeyModifiers::SHIFT),
        EditorCommand::ExtendLeft(1),
    );
    keymap.insert(
        key!(KeyCode::Right, KeyModifiers::SHIFT),
        EditorCommand::ExtendRight(1),
    );

    // ==========================================
    // BLOCK SELECTION MODE
    // ==========================================

    // Ctrl+V to start block selection (like Vim)
    keymap.insert(
        key!(KeyCode::Char('v'), KeyModifiers::CONTROL),
        EditorCommand::StartBlockSelection,
    );

    // Ctrl+Alt+V for block insert mode
    keymap.insert(
        key!(KeyCode::Char('v'), KeyModifiers::CONTROL | KeyModifiers::ALT),
        EditorCommand::BlockInsertMode,
    );

    // Escape to exit block mode
    keymap.insert(key!(KeyCode::Esc), EditorCommand::ExitBlockMode);

    // ==========================================
    // EDITING COMMANDS
    // ==========================================

    // Mode switching
    keymap.insert(key!('i'), EditorCommand::InsertMode);
    keymap.insert(key!('a'), EditorCommand::InsertMode); // Append (simplified for now)
    keymap.insert(key!('o'), EditorCommand::InsertMode); // Open below (simplified)

    // Delete operations
    keymap.insert(key!('x'), EditorCommand::Delete);
    keymap.insert(key!(KeyCode::Delete), EditorCommand::Delete);
    keymap.insert(key!(KeyCode::Backspace), EditorCommand::Backspace);

    // ==========================================
    // CLIPBOARD OPERATIONS
    // ==========================================

    keymap.insert(key!('y'), EditorCommand::Copy);
    keymap.insert(key!('d'), EditorCommand::Cut);
    keymap.insert(key!('p'), EditorCommand::Paste);

    // macOS-style clipboard
    keymap.insert(
        key!(KeyCode::Char('c'), KeyModifiers::SUPER),
        EditorCommand::Copy,
    );
    keymap.insert(
        key!(KeyCode::Char('x'), KeyModifiers::SUPER),
        EditorCommand::Cut,
    );
    keymap.insert(
        key!(KeyCode::Char('v'), KeyModifiers::SUPER),
        EditorCommand::Paste,
    );

    // ==========================================
    // UNDO/REDO
    // ==========================================

    keymap.insert(key!('u'), EditorCommand::Undo);
    keymap.insert(
        key!(KeyCode::Char('r'), KeyModifiers::CONTROL),
        EditorCommand::Redo,
    );

    // macOS-style undo/redo
    keymap.insert(
        key!(KeyCode::Char('z'), KeyModifiers::SUPER),
        EditorCommand::Undo,
    );
    keymap.insert(
        key!(KeyCode::Char('z'), KeyModifiers::SUPER | KeyModifiers::SHIFT),
        EditorCommand::Redo,
    );

    // ==========================================
    // CUSTOM MODE COMMANDS
    // ==========================================

    // Notes mode
    keymap.insert(
        key!(KeyCode::Char('n'), KeyModifiers::CONTROL),
        EditorCommand::CreateNote,
    );
    keymap.insert(
        key!(KeyCode::Char('f'), KeyModifiers::CONTROL),
        EditorCommand::SearchNotes,
    );

    // PDF mode
    keymap.insert(key!(KeyCode::Space), EditorCommand::NextPdfPage);
    keymap.insert(key!('b'), EditorCommand::PrevPdfPage);
    keymap.insert(
        key!(KeyCode::Char('e'), KeyModifiers::CONTROL),
        EditorCommand::ExtractPdfText,
    );

    keymap
}

/// Create keymap for insert mode
pub fn create_insert_keymap() -> KeyMap {
    let mut keymap = KeyMap::new();

    // Exit insert mode
    keymap.insert(key!(KeyCode::Esc), EditorCommand::NormalMode);

    // Movement in insert mode (arrows still work)
    keymap.insert(key!(KeyCode::Up), EditorCommand::MoveUp(1));
    keymap.insert(key!(KeyCode::Down), EditorCommand::MoveDown(1));
    keymap.insert(key!(KeyCode::Left), EditorCommand::MoveLeft(1));
    keymap.insert(key!(KeyCode::Right), EditorCommand::MoveRight(1));

    // Special keys in insert mode
    keymap.insert(key!(KeyCode::Enter), EditorCommand::InsertNewline);
    keymap.insert(key!(KeyCode::Backspace), EditorCommand::Backspace);
    keymap.insert(key!(KeyCode::Delete), EditorCommand::Delete);

    // Note: Regular character insertion is handled separately
    // as it doesn't need explicit mappings for each character

    keymap
}

/// Create keymap for block selection mode
pub fn create_block_keymap() -> KeyMap {
    let mut keymap = create_default_keymap(); // Start with normal bindings

    // Override some keys for block mode
    // Movement in block mode extends the selection
    keymap.insert(key!(KeyCode::Up), EditorCommand::ExtendBlockSelection);
    keymap.insert(key!(KeyCode::Down), EditorCommand::ExtendBlockSelection);
    keymap.insert(key!(KeyCode::Left), EditorCommand::ExtendBlockSelection);
    keymap.insert(key!(KeyCode::Right), EditorCommand::ExtendBlockSelection);

    // Vim-style movement also extends in block mode
    keymap.insert(key!('h'), EditorCommand::ExtendBlockSelection);
    keymap.insert(key!('j'), EditorCommand::ExtendBlockSelection);
    keymap.insert(key!('k'), EditorCommand::ExtendBlockSelection);
    keymap.insert(key!('l'), EditorCommand::ExtendBlockSelection);

    // 'I' for block insert at beginning of selection
    keymap.insert(key!('I'), EditorCommand::BlockInsertMode);

    // 'A' for block append at end of selection
    keymap.insert(key!('A'), EditorCommand::BlockInsertMode);

    // Exit block mode
    keymap.insert(key!(KeyCode::Esc), EditorCommand::ExitBlockMode);

    keymap
}

// Helper macro for creating key events
macro_rules! key {
    ($key:expr) => {
        KeyEvent {
            code: $key,
            modifiers: KeyModifiers::NONE,
        }
    };
    ($key:expr, $mods:expr) => {
        KeyEvent {
            code: $key,
            modifiers: $mods,
        }
    };
    // For character keys
    ($char:literal) => {
        KeyEvent {
            code: KeyCode::Char($char),
            modifiers: KeyModifiers::NONE,
        }
    };
    // For multi-key sequences (simplified for now)
    ($first:literal, $second:literal) => {
        KeyEvent {
            code: KeyCode::Char($first),
            modifiers: KeyModifiers::NONE,
        }
    };
}

pub(crate) use key;

/// Handle keyboard input based on current mode
pub fn handle_key_input(
    editor: &mut HelixNativeEditor,
    key: KeyEvent,
) -> anyhow::Result<()> {
    // Select keymap based on current mode
    let keymap = match editor.selection_mode {
        SelectionMode::Normal => create_default_keymap(),
        SelectionMode::Block => create_block_keymap(),
        SelectionMode::BlockInsert => create_insert_keymap(),
    };

    // Look up and execute command
    if let Some(command) = keymap.get(&key) {
        editor.handle_command(command.clone())?;
    } else {
        // Handle character insertion in insert modes
        if matches!(editor.selection_mode, SelectionMode::BlockInsert) {
            if let KeyCode::Char(c) = key.code {
                editor.handle_command(EditorCommand::InsertChar(c))?;
            }
        }
    }

    Ok(())
}

/// Get a description of what a key does in the current mode
pub fn describe_key(key: KeyEvent, mode: SelectionMode) -> String {
    let keymap = match mode {
        SelectionMode::Normal => create_default_keymap(),
        SelectionMode::Block => create_block_keymap(),
        SelectionMode::BlockInsert => create_insert_keymap(),
    };

    if let Some(command) = keymap.get(&key) {
        format!("{:?}", command)
    } else {
        "Unmapped".to_string()
    }
}