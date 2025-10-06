# chonk-note

A lightweight, terminal-based notes editor built with Rust, featuring Helix editor's text manipulation core and a spatial grid-based editing system.

## 🎯 Current State

chonk-note is a functional terminal notes application that provides a distraction-free writing environment with persistent storage. It's designed for developers who live in the terminal and want a fast, keyboard-driven notes system without leaving their workflow.

**Historical Note**: This project evolved from Chonker7 (a PDF viewer). The `/lib/libpdfium.dylib.old` and some references to PDF functionality are remnants from that previous incarnation. The current focus is entirely on note-taking.

## ✨ Features

### Core Functionality

- 📝 **SQLite-backed storage** - All notes are persisted in a local database
- 🎯 **Helix-powered editing** - Uses Helix editor's core for robust text manipulation
- 📋 **Block selection** - Vim-style visual block mode for column editing
- 🖱️ **Mouse support** - Click to position cursor, select text
- 📑 **Sidebar navigation** - Collapsible notes list with keyboard navigation
- ⚡ **Fast & lightweight** - Instant startup, minimal dependencies

### Editing Features

- **Virtual grid cursor** - Move cursor beyond text boundaries (useful for ASCII art/tables)
- **Smart text wrapping** - Toggle between wrapped and unwrapped display
- **Block clipboard** - Copy/paste rectangular text selections
- **Undo/redo history** - Full edit history per note
- **Title editing** - Inline title editing for better organization

### UI/UX

- **Flicker-free rendering** - Synchronized terminal updates at 20 FPS
- **Responsive layout** - Adapts to terminal resizing
- **Status messages** - Contextual hints and feedback
- **Color-coded interface** - Visual hierarchy with syntax highlighting

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
- A terminal emulator with:
  - 256 color support
  - UTF-8 encoding
  - Mouse support (optional but recommended)
- Kitty terminal recommended for best experience

## ⌨️ Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `Ctrl+N` | Create new note |
| `Ctrl+↑/↓` | Navigate between notes |
| Arrow keys | Move cursor |

### Editing

| Key | Action |
|-----|--------|
| `Cmd+C` | Copy selection |
| `Cmd+X` | Cut selection |
| `Cmd+V` | Paste |
| `Cmd+A` | Select all |
| `Backspace` | Delete character |
| `Enter` | New line |

### Application

| Key | Action |
|-----|--------|
| `Ctrl+Q` | Quit application |
| `Esc` | Cancel current operation |

## 🗂️ Project Structure

```
chonk-note/
├── src/
│   ├── main.rs                 # Application entry point
│   ├── edit_renderer.rs        # Terminal rendering engine
│   ├── keyboard.rs             # Keyboard input handling
│   ├── mouse.rs                # Mouse event processing
│   ├── block_selection.rs      # Visual block mode
│   ├── notes_database.rs       # SQLite persistence layer
│   ├── notes_mode.rs           # Notes management logic
│   ├── virtual_grid.rs         # Spatial text grid
│   ├── grid_cursor.rs          # Cursor positioning system
│   ├── kitty_native.rs         # Terminal abstraction
│   └── debug.rs                # Debug logging utilities
└── Cargo.toml                   # Dependencies and build config
```

## 🔧 Technical Details

### Core Technologies

- **Language**: Rust
- **Text Engine**: Helix-core (rope data structure for efficient text manipulation)
- **Database**: SQLite with rusqlite bindings
- **Terminal UI**: Custom ANSI escape sequence renderer
- **Async Runtime**: Tokio for non-blocking I/O

### Design Decisions

- **No TUI framework**: Direct terminal control for better performance
- **Rope-based editing**: Efficient for large texts and complex operations
- **Virtual grid system**: Allows cursor positioning beyond text boundaries
- **20 FPS cap**: Balances responsiveness with CPU usage
- **Synchronized updates**: Prevents screen tearing and flicker

## 📁 Data Storage

Notes are stored in a SQLite database at:

- **macOS**: `~/Library/Application Support/chonk-note/notes.db`
- **Linux**: `~/.local/share/chonk-note/notes.db`
- **Windows**: `%APPDATA%\chonk-note\notes.db`

Each note contains:
- Unique SHA-256 ID
- Title (editable)
- Content (UTF-8 text)
- Creation timestamp
- Last modified timestamp

## 🚧 Current Limitations & Future Work

### Known Limitations

- No search functionality across notes yet
- No export options (Markdown, plain text)
- No tags or categories system
- Single database only (no sync/multiple profiles)
- Limited to terminal environments

### Potential Enhancements

- [ ] Full-text search with ripgrep integration
- [ ] Note templates
- [ ] Markdown preview mode
- [ ] Export to various formats
- [ ] Tag system with filtering
- [ ] Vim keybinding mode
- [ ] Encrypted notes option
- [ ] Multiple database support
- [ ] Config file for customization

## 🐛 Debugging

Debug logs are written to `/tmp/chonk-debug.log` during runtime. Enable verbose logging by setting:

```bash
export CHONK_DEBUG=1
```

## 📊 Statistics

- **Total lines of code**: ~2,800
- **Core files**: 11
- **Dependencies**: Minimal (helix-core, rusqlite, tokio)
- **Build time**: <5 seconds
- **Binary size**: ~2MB (optimized release)

## 📜 License

MIT License - see LICENSE file for details

## 🤝 Contributing

This is a personal project, but suggestions and bug reports are welcome via GitHub issues.
