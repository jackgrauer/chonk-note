# Helix-Native Architecture for Chonker7

## Core Problem: Stop Fighting Helix, Start Using It

Your current implementation manually manipulates Rope and Selection, reimplementing what Helix already provides. Let's use Helix's actual architecture.

## Helix's Real Architecture (What We Should Use)

### 1. Document & View (Not Raw Rope)
```rust
use helix_view::{Document, View, Editor, ViewId};
use helix_core::{syntax, Selection};

// WRONG - What you're doing now
struct App {
    rope: Rope,
    selection: Selection,
    // manually tracking everything
}

// RIGHT - Use Helix's abstractions
struct App {
    editor: Editor,  // Manages documents and views
    // Editor already handles:
    // - Multiple documents
    // - Undo/redo history
    // - Selections per view
    // - Syntax highlighting
    // - Auto-indentation
    // - And much more!
}
```

### 2. Commands (Not Match Statements)
```rust
use helix_term::commands::{self, Command, MappableCommand};
use helix_view::input::KeyEvent;

// WRONG - Current approach
match key {
    KeyCode::Left => {
        // 50 lines of manual movement code
    }
}

// RIGHT - Use Helix commands
fn setup_keymap() -> HashMap<KeyEvent, MappableCommand> {
    let mut keymap = HashMap::new();

    // Use Helix's built-in commands
    keymap.insert(key!('h'), MappableCommand::Simple(commands::move_char_left));
    keymap.insert(key!('j'), MappableCommand::Simple(commands::move_line_down));
    keymap.insert(key!('k'), MappableCommand::Simple(commands::move_line_up));
    keymap.insert(key!('l'), MappableCommand::Simple(commands::move_char_right));

    // Or create custom commands that work with Context
    keymap.insert(key!('ctrl-n'), MappableCommand::Simple(create_note));

    keymap
}
```

### 3. Context (Not Direct Manipulation)
```rust
use helix_view::editor::Context;

// WRONG - Direct manipulation
let pos = app.selection.primary().head;
let line = app.rope.char_to_line(pos);
// ... manual calculation ...

// RIGHT - Use Context and helper functions
fn move_to_line_start(cx: &mut Context) {
    let (view, doc) = current!(cx.editor);
    let selection = doc.selection(view.id).clone().transform(|range| {
        let line = range.head_line(doc.text().slice(..));
        let pos = doc.text().line_to_char(line);
        Range::point(pos)
    });
    doc.set_selection(view.id, selection);
}
```

### 4. Transactions (Not Direct Edits)
```rust
use helix_core::{Transaction, ChangeSet, Operation};

// WRONG - Manual rope manipulation
let mut rope = self.rope.clone();
rope.insert(pos, "text");
self.rope = rope;

// RIGHT - Use transactions
fn insert_text(cx: &mut Context, text: &str) {
    let (view, doc) = current!(cx.editor);
    let transaction = Transaction::insert(
        doc.text(),
        doc.selection(view.id),
        text
    );
    doc.apply(&transaction, view.id);
    // Undo history is automatically managed!
}
```

## Proper Helix-Native Implementation

### Core Structure
```rust
// src/core/editor.rs
use helix_view::{Editor, Document, View, Theme};
use helix_core::{syntax, config::Config};
use helix_lsp::Registry;

pub struct HelixNativeEditor {
    editor: Editor,
    config: Config,
    keymap: KeyMap,

    // Mode-specific extensions
    mode: AppMode,
}

impl HelixNativeEditor {
    pub fn new() -> Result<Self> {
        let config = Config::default();
        let theme = Theme::default();
        let syn_loader = syntax::Loader::new(config.clone());
        let editor = Editor::new(
            Box::new(EditorView::new()),
            syn_loader,
            config.clone(),
            theme,
        );

        Ok(Self {
            editor,
            config,
            keymap: create_default_keymap(),
            mode: AppMode::Editor,
        })
    }

    pub fn handle_input(&mut self, event: KeyEvent) -> Result<()> {
        // Look up command in keymap
        if let Some(command) = self.keymap.get(&event) {
            let mut cx = Context {
                editor: &mut self.editor,
                count: 1,
                callback: None,
            };
            command.execute(&mut cx)?;
        }
        Ok(())
    }
}
```

### Command Implementation
```rust
// src/core/commands.rs
use helix_view::editor::Context;
use helix_term::commands;

// Re-export Helix's commands
pub use helix_term::commands::{
    move_char_left, move_char_right,
    move_line_up, move_line_down,
    move_next_word_start, move_prev_word_end,
    insert_mode, normal_mode,
    undo, redo,
    // ... dozens more already implemented!
};

// Add our custom commands
pub fn create_note(cx: &mut Context) {
    // This works WITH Helix, not against it
    let (view, doc) = current!(cx.editor);

    // Create new document using Helix's document management
    let note_id = generate_note_id();
    let note_path = note_path_from_id(note_id);

    // Let Helix handle the document
    cx.editor.open(note_path, Action::Replace)?;
}

pub fn search_notes(cx: &mut Context) {
    // Use Helix's picker UI
    let picker = FilePicker::new(
        list_notes()?,
        |note| note.title.clone(),
        |cx, note, _action| {
            cx.editor.open(note.path, Action::Replace)?;
        }
    );
    cx.push_layer(Box::new(picker));
}

pub fn extract_pdf_text(cx: &mut Context) {
    if let AppMode::Pdf { document, .. } = &cx.editor.mode {
        let text = document.extract_text()?;

        // Create new document with extracted text
        let doc = Document::from_text(text);
        cx.editor.new_document(doc);
    }
}
```

### Keymap Configuration
```rust
// src/core/keymap.rs
use helix_view::input::{KeyEvent, KeyCode, KeyModifiers};

pub fn create_default_keymap() -> KeyMap {
    let mut keymap = KeyMap::new();

    // Normal mode - Helix defaults
    keymap.insert(Mode::Normal, key!('h'), move_char_left);
    keymap.insert(Mode::Normal, key!('j'), move_line_down);
    keymap.insert(Mode::Normal, key!('k'), move_line_up);
    keymap.insert(Mode::Normal, key!('l'), move_char_right);

    keymap.insert(Mode::Normal, key!('w'), move_next_word_start);
    keymap.insert(Mode::Normal, key!('b'), move_prev_word_end);

    keymap.insert(Mode::Normal, key!('i'), insert_mode);
    keymap.insert(Mode::Normal, key!('a'), append_mode);

    keymap.insert(Mode::Normal, key!('u'), undo);
    keymap.insert(Mode::Normal, key!('ctrl-r'), redo);

    // Insert mode
    keymap.insert(Mode::Insert, key!('esc'), normal_mode);

    // Custom commands for modes
    keymap.insert(Mode::Normal, key!('ctrl-n'), create_note);
    keymap.insert(Mode::Normal, key!('ctrl-f'), search_notes);
    keymap.insert(Mode::Normal, key!('ctrl-e'), extract_pdf_text);

    keymap
}

pub fn create_notes_keymap() -> KeyMap {
    let mut keymap = create_default_keymap();

    // Notes-specific bindings
    keymap.insert(Mode::Normal, key!('space-n'), new_note);
    keymap.insert(Mode::Normal, key!('space-s'), search_notes);
    keymap.insert(Mode::Normal, key!('space-c'), apply_qda_code);

    keymap
}

pub fn create_pdf_keymap() -> KeyMap {
    let mut keymap = create_default_keymap();

    // PDF-specific bindings
    keymap.insert(Mode::Normal, key!('space'), next_page);
    keymap.insert(Mode::Normal, key!('b'), previous_page);
    keymap.insert(Mode::Normal, key!('o'), run_ocr);
    keymap.insert(Mode::Normal, key!('e'), extract_text);

    keymap
}
```

### Mode Integration
```rust
// src/modes/mod.rs
pub enum AppMode {
    Editor,
    Notes { db: NotesDb },
    Pdf { doc: PdfDocument },
}

impl AppMode {
    pub fn keymap(&self) -> KeyMap {
        match self {
            AppMode::Editor => create_default_keymap(),
            AppMode::Notes { .. } => create_notes_keymap(),
            AppMode::Pdf { .. } => create_pdf_keymap(),
        }
    }

    pub fn handle_command(&mut self, cx: &mut Context, cmd: Command) -> Result<()> {
        // Mode-specific command handling
        match self {
            AppMode::Notes { db } => {
                // Notes-specific commands have access to db
            }
            AppMode::Pdf { doc } => {
                // PDF-specific commands have access to document
            }
            _ => {}
        }

        // Execute the command
        cmd.execute(cx)
    }
}
```

## Migration Path from Current Code

### Phase 1: Setup Helix Foundation (Day 1-3)
1. Create `Context` wrapper around current App
2. Import Helix's command infrastructure
3. Set up Document/View instead of raw Rope

### Phase 2: Convert Commands (Day 4-7)
```rust
// Convert this:
KeyCode::Left => {
    let pos = app.selection.primary().head;
    // 20 lines of manual movement
}

// To this:
key!('h') => commands::move_char_left(&mut cx),
```

### Phase 3: Leverage Helix Features (Week 2)
- Use Helix's undo tree (not custom history)
- Use Helix's syntax highlighting
- Use Helix's auto-indent
- Use Helix's text objects
- Use Helix's multiple cursors
- Use Helix's jump list

## Benefits of Going Helix-Native

### What You Get for Free
1. **Professional Movement Commands**: All Vim-like movements, text objects, etc.
2. **Undo/Redo Tree**: Branching history with timestamps
3. **Multiple Cursors**: Already implemented and tested
4. **Syntax Highlighting**: Tree-sitter integration
5. **Auto-indentation**: Language-aware indenting
6. **LSP Support**: If you want it later
7. **Incremental Parsing**: Efficient for large files
8. **Macros**: Record and replay
9. **Registers**: Yank rings, named registers
10. **Search & Replace**: Regex, incremental search

### What You Stop Maintaining
- Manual cursor movement calculations
- Custom selection handling
- History management
- Virtual column tracking
- Line/character conversions
- Boundary checking
- And ~500+ lines of manual manipulation code

## Example: Current vs Helix-Native

### Current Approach (keyboard.rs - 50 lines)
```rust
(KeyCode::Up, mods) => {
    app.block_selection = None;
    let pos = app.selection.primary().head;
    let line = app.rope.char_to_line(pos);
    let lines_to_move = 1.min(line);

    if lines_to_move > 0 {
        let virtual_col = if let Some(vc) = app.virtual_cursor_col {
            vc
        } else {
            let line_start = app.rope.line_to_char(line);
            pos - line_start
        };

        let new_line = line - lines_to_move;
        let line_start = app.rope.line_to_char(new_line);
        let line_len = app.rope.line(new_line).len_chars().saturating_sub(1);
        let new_pos = line_start + virtual_col.min(line_len);

        if mods.contains(KeyModifiers::SHIFT) {
            let anchor = app.selection.primary().anchor;
            app.selection = Selection::single(anchor, new_pos);
        } else {
            app.selection = Selection::point(new_pos);
        }

        app.virtual_cursor_col = None;
    }
}
```

### Helix-Native (1 line)
```rust
key!('k') => commands::move_line_up(&mut cx),
// Virtual columns, selections, boundaries - all handled!
```

## Next Steps

1. **Start Fresh**: Don't try to adapt current code, rebuild on Helix
2. **Use helix-term**: It has the commands you need
3. **Study Helix Source**: See how they implement features
4. **Incremental Migration**: Keep old code running while building new

The key insight: **Helix is not just a text manipulation library, it's a complete editor framework**. Use it as one!