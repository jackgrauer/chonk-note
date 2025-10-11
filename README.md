# chonk-note

A lightweight, terminal-based notes editor built with Rust, featuring a chunked grid system and Microsoft Word-style text editing.

## 🎯 Current State

chonk-note is a functional terminal notes application that provides a distraction-free writing environment with persistent storage. It's designed for developers who live in the terminal and want a fast, keyboard-driven notes system without leaving their workflow.

**Historical Note**: This project evolved from Chonker7 (a PDF viewer). The `/lib/libpdfium.dylib.old` and some references to PDF functionality are remnants from that previous incarnation. The current focus is entirely on note-taking.

## ✨ Features

### Core Functionality

- 📝 **SQLite-backed storage** - All notes are persisted in a local database
- 🎯 **Chunked grid editing** - Efficient sparse grid system for text manipulation
- 📋 **Block selection** - Visual block mode with mouse drag support
- 🖱️ **Full mouse support** - Click to position cursor, drag to select, scroll notes list
- 📑 **Sidebar navigation** - Collapsible notes list with mouse and keyboard navigation
- ⚡ **Fast & lightweight** - Instant startup, native Kitty terminal integration

### Editing Features

- **Microsoft Word-style editing** - Insert mode with character shifting, line splitting/joining
- **Full undo/redo** - Complete undo stack for all editing operations (Ctrl+Z/Ctrl+Y)
- **Virtual grid cursor** - Move cursor anywhere on the infinite grid
- **Block clipboard** - Copy/cut/paste rectangular text selections with system clipboard integration
- **Search functionality** - Full-text search within current note (Ctrl+F)
- **Double-click rename** - Double-click notes in sidebar to rename
- **Auto-save** - Notes save automatically every 2 seconds when modified
- **Settings panel** - Toggle soft-wrapped paste, grid lines, and other options
- **Grid lines toggle** - Optional visual grid overlay (Ctrl+G)

### UI/UX

- **Kitty graphics protocol** - Displays emoji as inline PNG images
- **60 FPS rendering** - Smooth mouse drag selection
- **Responsive layout** - Adapts to terminal resizing
- **Status messages** - Contextual hints and feedback
- **Color-coded interface** - Yellow title bar, blue sidebar, pink selection highlights

## 🚀 Installation

```bash
# Clone the repository
git clone https://github.com/jackgrauer/chonk-note.git
cd chonk-note

# Build and install (requires Rust toolchain)
cargo build --release

# Optional: Copy to system path
sudo cp target/release/chonk-note /usr/local/bin/

# Or run directly
./target/release/chonk-note
```

## 📋 Requirements

- Rust 1.70+
- Kitty terminal emulator (required for graphics protocol and mouse support)
- macOS, Linux, or Windows with WSL

## ⌨️ Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `Ctrl+N` | Create new note (or next search result if searching) |
| `Ctrl+↑/↓` | Navigate between notes |
| Arrow keys | Move cursor |
| `Ctrl+Q` | Quit application |

### Editing

| Key | Action |
|-----|--------|
| `Ctrl+C` | Copy selection to system clipboard |
| `Ctrl+X` | Cut selection to system clipboard |
| `Ctrl+V` | Paste from system clipboard |
| `Ctrl+A` | Select all |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` or `Ctrl+Shift+Z` | Redo |
| `Backspace` | Delete character before cursor (Word-style) |
| `Delete` | Delete character at cursor (Word-style) |
| `Enter` | Split line at cursor (Word-style) |
| `Esc` | Clear selection |

### Search

| Key | Action |
|-----|--------|
| `Ctrl+F` | Start search mode |
| `Ctrl+N` | Next search result (when results exist) |
| `Ctrl+P` | Previous search result (when results exist) |
| `Enter` | Jump to first result and exit search |
| `Esc` | Cancel search |

### View

| Key | Action |
|-----|--------|
| `Ctrl+G` | Toggle grid lines |
| `Ctrl+S` | Manual save (auto-save every 2 seconds) |

### Note Management

| Key | Action |
|-----|--------|
| `Ctrl+D` | Delete current note (press twice to confirm) |
| Double-click note | Enter rename mode |

## 🖱️ Mouse Controls

- **Click in editor** - Position cursor
- **Click in sidebar** - Switch to note (expands sidebar if collapsed)
- **Double-click note** - Rename note
- **Drag in editor** - Block selection
- **Scroll in sidebar** - Scroll notes list
- **Scroll in editor** - Scroll viewport up/down
- **Click "Notes ▾"** - Toggle notes sidebar and dropdown menu
- **Click "Settings ▾"** - Toggle settings panel and dropdown menu
- **Click settings toggles** - Toggle soft-wrapped paste, grid lines, etc.

## 🗂️ Project Structure

```
chonk-note/
├── src/
│   ├── main.rs                 # Application entry point and rendering
│   ├── keyboard.rs             # Keyboard input handling
│   ├── mouse.rs                # Mouse event processing
│   ├── chunked_grid.rs         # Sparse grid with block selection
│   ├── notes_database.rs       # SQLite persistence layer
│   ├── notes_mode.rs           # Notes management logic
│   ├── undo.rs                 # Undo/redo system
│   ├── config.rs               # Configuration constants and colors
│   └── kitty_native.rs         # Kitty terminal protocol
├── assets/
│   └── hamster.png             # Hamster emoji for title bar
└── Cargo.toml                  # Dependencies and build config
```

## 🔧 Technical Details

### Core Technologies

- **Language**: Rust
- **Grid System**: Chunked sparse grid (1000x1000 chunks)
- **Database**: SQLite with rusqlite bindings
- **Terminal Protocol**: Kitty native (ANSI + Kitty extensions)
- **Async Runtime**: Tokio for non-blocking I/O
- **Graphics**: Kitty graphics protocol for PNG rendering

### Design Decisions

- **No TUI framework**: Direct terminal control for better performance
- **Chunked grid**: Efficient sparse storage with O(1) access
- **Word-style editing**: Familiar text manipulation behavior
- **60 FPS updates**: Smooth drag selection with frame limiting
- **Kitty-native**: Leverages Kitty's advanced features (graphics, mouse, etc.)

## 📁 Data Storage

Notes are stored in a SQLite database at:

- **macOS**: `~/Library/Application Support/chonk-note/notes.db`
- **Linux**: `~/.local/share/chonk-note/notes.db`
- **Windows**: `%APPDATA%\chonk-note\notes.db`

Each note contains:
- Unique SHA-256 ID
- Title (editable)
- Content (stored as lines)
- Creation timestamp
- Last modified timestamp
- Tags (array, currently unused)

## 🚧 Current Limitations & Future Work

### Known Limitations

- Requires Kitty terminal (no fallback for other terminals)
- No export options (Markdown, plain text)
- No tags system implementation
- Single database only (no sync/multiple profiles)
- No syntax highlighting or Markdown rendering
- Search is limited to current note only (not across all notes)

### Potential Enhancements

- [ ] Full-text search across all notes (currently only searches within active note)
- [ ] Tag system with filtering
- [ ] Export to Markdown/plain text
- [ ] Import from existing files
- [ ] Markdown preview mode
- [ ] Multiple database profiles
- [ ] Vim keybinding mode
- [ ] Encrypted notes option
- [ ] Config file for customization
- [ ] Improved word wrapping with configurable width

## 🐛 Debugging

Debug logs are written to `/tmp/chonk-debug.log` during runtime. This includes:
- Mouse event coordinates
- Selection state changes
- Application lifecycle events

## 📊 Statistics

- **Total lines of code**: ~1,500
- **Core files**: 7
- **Dependencies**: Minimal (rusqlite, tokio, arboard, sha2, base64)
- **Build time**: <3 seconds
- **Binary size**: ~1.5MB (optimized release)

## 📜 License

MIT License - see LICENSE file for details

## 🤝 Contributing

This is a personal project, but suggestions and bug reports are welcome via GitHub issues.

---

Made with 🐹 and Rust
