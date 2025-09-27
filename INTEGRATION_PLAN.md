# Chonker7 Integration Plan: Unified Text Editor & PDF Viewer

## Executive Summary
Merge snyfter3 (note-taking app) and chonker7 (PDF viewer/editor) into a unified Chonker7 application with multiple operational modes, sharing a common text editing core based on Helix architecture.

## Current State Analysis

### Snyfter3 Capabilities
- **Note Management**: SQLite-based note storage with metadata
- **Fuzzy Search**: High-performance nucleo-based note finder
- **QDA Features**: Qualitative data analysis with coding system
- **Syntax Highlighting**: Syntect-based code highlighting
- **Text Editing**: Helix-core based editor

### Chonker7 Capabilities
- **PDF Rendering**: pdfium-render for PDF display
- **OCR**: Tesseract integration via leptess
- **Image Display**: Terminal image rendering with viuer
- **File Picker**: Kitty-native file selection
- **Text Editing**: Helix-core based editor (duplicate)

### Shared Components (Duplicated)
- Helix-core text editing (~700 lines keyboard.rs each)
- Kitty native terminal integration
- Block selection logic
- Edit renderer
- Crossterm UI handling

## Unified Architecture Design

### Core Architecture
```
chonker7/
├── src/
│   ├── main.rs                 # Mode router & CLI
│   ├── app.rs                   # Unified App state
│   │
│   ├── core/                    # Shared foundation
│   │   ├── mod.rs
│   │   ├── editor.rs            # Helix-based text editor
│   │   ├── commands.rs          # Command pattern implementation
│   │   ├── keymap.rs            # Configurable key bindings
│   │   ├── terminal.rs          # Kitty/terminal abstraction
│   │   └── ui.rs                # Shared UI components
│   │
│   ├── modes/                   # Application modes
│   │   ├── mod.rs
│   │   ├── notes/               # From snyfter3
│   │   │   ├── mod.rs
│   │   │   ├── database.rs      # SQLite note storage
│   │   │   ├── search.rs        # Fuzzy finder
│   │   │   └── qda.rs           # QDA coding system
│   │   │
│   │   ├── pdf/                 # From chonker7
│   │   │   ├── mod.rs
│   │   │   ├── renderer.rs      # PDF rendering
│   │   │   ├── ocr.rs           # OCR integration
│   │   │   └── extractor.rs     # Content extraction
│   │   │
│   │   └── editor/              # Pure text editing mode
│   │       └── mod.rs
│   │
│   └── utils/                   # Utilities
│       ├── clipboard.rs
│       ├── file_picker.rs
│       └── image_display.rs
```

### Mode System
```rust
enum AppMode {
    Notes {
        db: NotesDatabase,
        search_engine: NucleoSearch,
        qda_system: QdaCodes,
    },
    Pdf {
        document: PdfDocument,
        renderer: PdfRenderer,
        ocr: Option<OcrEngine>,
    },
    Editor {
        // Pure text editing, no special features
    },
}

struct UnifiedApp {
    mode: AppMode,
    editor: TextEditor,  // Shared Helix-based editor
    terminal: Terminal,  // Shared terminal handling
    keymap: KeyMap,      // Mode-specific keymaps
}
```

### Command Pattern Architecture
```rust
// Instead of giant match statements
trait Command {
    fn execute(&self, ctx: &mut Context) -> Result<()>;
    fn undo(&self, ctx: &mut Context) -> Result<()>;
}

enum EditorCommand {
    // Movement
    Move(Movement),
    Jump(Location),

    // Editing
    Insert(String),
    Delete(Range),
    Replace(Range, String),

    // Selection
    Select(SelectionMode),
    ExtendSelection(Direction),

    // Mode-specific
    ModeSpecific(Box<dyn Command>),
}

// Mode-specific commands
enum NotesCommand {
    CreateNote,
    SearchNotes,
    ApplyCode(QdaCode),
}

enum PdfCommand {
    NextPage,
    PreviousPage,
    ExtractText,
    RunOcr(Region),
}
```

## Migration Plan

### Phase 1: Foundation (Week 1)
**Goal**: Create unified project structure with shared core

1. **Day 1-2**: Project Setup
   - Create new Chonker7 structure
   - Merge Cargo.toml dependencies
   - Set up core module structure

2. **Day 3-4**: Extract Shared Core
   - Move Helix integration to core/editor.rs
   - Unify terminal handling (kitty_native.rs)
   - Merge UI components

3. **Day 5-7**: Command Pattern
   - Implement command trait system
   - Convert keyboard.rs to commands
   - Create keymap configuration

**Deliverable**: Working text editor with command pattern

### Phase 2: Mode Integration (Week 2)
**Goal**: Integrate both apps as separate modes

1. **Day 8-9**: Notes Mode
   - Port snyfter3 note storage
   - Integrate fuzzy search
   - Port QDA system

2. **Day 10-11**: PDF Mode
   - Port PDF rendering
   - Integrate OCR
   - Port content extraction

3. **Day 12-14**: Mode Switching
   - Implement mode router
   - Add mode-specific keymaps
   - Test mode transitions

**Deliverable**: Unified app with working modes

### Phase 3: Optimization (Week 3)
**Goal**: Remove duplication and optimize

1. **Day 15-16**: Deduplication
   - Remove redundant code
   - Unify similar functions
   - Consolidate utilities

2. **Day 17-18**: Performance
   - Profile and optimize
   - Lazy loading for modes
   - Memory management

3. **Day 19-21**: Polish
   - Unified configuration
   - Documentation
   - Testing

**Deliverable**: Production-ready Chonker7

## Implementation Strategy

### Step 1: Initial Setup
```bash
# Backup existing projects
cp -r chonker7 chonker7_backup
cp -r snyfter3 snyfter3_backup

# Create new structure in chonker7
cd chonker7
mkdir -p src/core src/modes/notes src/modes/pdf src/modes/editor src/utils
```

### Step 2: Core Extraction Script
```rust
// src/core/editor.rs - Unified editor from both apps
pub struct TextEditor {
    rope: Rope,
    selection: Selection,
    history: History,
    viewport: Viewport,
}

impl TextEditor {
    pub fn handle_command(&mut self, cmd: EditorCommand) -> Result<()> {
        // Unified command handling
    }
}
```

### Step 3: Mode Implementation
```rust
// src/modes/mod.rs
pub trait Mode {
    fn name(&self) -> &str;
    fn handle_command(&mut self, cmd: Command, ctx: &mut Context) -> Result<()>;
    fn render(&self, terminal: &mut Terminal) -> Result<()>;
    fn keymaps(&self) -> &KeyMap;
}
```

### Step 4: Gradual Migration
1. Start with working chonker7
2. Add notes module alongside existing code
3. Gradually move PDF features to new structure
4. Remove old code once new structure works

## Configuration Management

### Unified Config File
```toml
# ~/.config/chonker7/config.toml

[general]
default_mode = "notes"
theme = "dark"

[editor]
tab_width = 4
line_numbers = true
auto_save = true

[modes.notes]
db_path = "~/.local/share/chonker7/notes.db"
autosave_interval = 30

[modes.pdf]
ocr_enabled = true
cache_extracted_text = true

[keymaps]
# Mode-agnostic keys
"ctrl-q" = "quit"
"ctrl-s" = "save"

[keymaps.notes]
"ctrl-n" = "new_note"
"ctrl-f" = "search_notes"

[keymaps.pdf]
"space" = "next_page"
"b" = "previous_page"
```

## Testing Strategy

### Unit Tests
- Command execution tests
- Mode transition tests
- Editor operation tests

### Integration Tests
- Note creation and retrieval
- PDF rendering and extraction
- Mode switching scenarios

### Migration Tests
- Data migration from snyfter3 DB
- Config migration
- Backward compatibility

## Risk Mitigation

### Risks & Mitigations
1. **Data Loss**: Keep backups, implement migration tools
2. **Feature Regression**: Comprehensive test suite
3. **Performance Degradation**: Profile before/after
4. **User Confusion**: Clear migration guide

### Rollback Plan
- Keep old binaries available
- Version data formats
- Provide downgrade path

## Success Criteria

### Must Have
- [ ] All snyfter3 features work in notes mode
- [ ] All chonker7 PDF features work in PDF mode
- [ ] No performance regression
- [ ] Unified command system
- [ ] Single binary distribution

### Nice to Have
- [ ] Cross-mode features (annotate PDFs with notes)
- [ ] Plugin system for future modes
- [ ] Config hot-reload
- [ ] Mode-specific themes

## Timeline Summary

- **Week 1**: Foundation and core extraction
- **Week 2**: Mode integration
- **Week 3**: Optimization and polish
- **Week 4**: Testing and documentation

Total: 4 weeks to production-ready unified Chonker7

## Next Steps

1. Review and approve this plan
2. Set up new project structure
3. Begin Phase 1 implementation
4. Create migration scripts for existing users

---

*This plan prioritizes minimal disruption while achieving maximum code reuse and maintainability.*