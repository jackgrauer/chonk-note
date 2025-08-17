# Chonker7 v7.64.0 - Terminal PDF Viewer/Editor Specification

## Core Purpose
A terminal-based PDF viewer/editor that extracts text from PDFs and displays it in an editable character grid, supporting both text editing and markdown rendering modes.

## Architecture Overview

### Display Modes
1. **PdfText Mode** - Editable text grid with spatial layout preservation
2. **PdfReader Mode** - Formatted markdown rendering of PDF content  
3. **Debug Mode** - Console for debugging and ML stats

### Two-Pass Processing System
1. **Pass 1**: Extract raw character data with positions, fonts, colors
2. **Pass 2**: Apply ML models for entity recognition, table detection, layout analysis

## Core Components

### PDF Processing Pipeline
```
PDF File → PDFium Extraction → Character Grid (width×height) → Display
         ↓
    Character Data:
    - Position (x, y)
    - Font size
    - Color
    - Text content
    - Baseline/rotation
```

### Key Modules
- `content_extractor.rs` - Main PDF text extraction using PDFium
- `two_pass.rs` - Two-pass architecture with caching
- `edit_renderer.rs` - Text editing UI with cursor, selection, clipboard
- `markdown_renderer.rs` - Markdown formatting for reader mode
- `pdf_renderer.rs` - PDF page rendering to images
- `viuer_display.rs` - Terminal image display (fallback when text fails)

### ML Features (Optional)
- `ml/` - LayoutLM v3 for document understanding
  - Entity recognition (titles, headers, paragraphs)
  - Table structure detection
  - Layout region classification
- `ocr/` - OCR support for scanned PDFs (ocrs/rten)

## Text Extraction Strategy

### Primary Method: PDFium Direct Extraction
1. Extract all characters with precise coordinates
2. Group characters into words using spatial proximity
3. Detect columns based on vertical gaps
4. Map to fixed-size character grid preserving layout

### Fallback Methods
1. If PDFium fails → Show PDF as image (viuer)
2. If scanned PDF → OCR extraction (if enabled)
3. If all fails → Empty grid with error message

## Key Dependencies

### Core
- `pdfium-render` - PDF parsing and rendering (requires libpdfium.dylib)
- `crossterm` - Terminal UI without tearing (no ratatui!)
- `tokio` - Async runtime

### UI/Display
- `viuer` - Terminal image display fallback
- `termimad` - Markdown rendering
- `cli-clipboard` - System clipboard integration

### Text Layout
- `rstar` - R-tree spatial indexing for text positioning
- `euclid` - 2D geometry calculations
- `ordered-float` - Float ordering for spatial sorting

### ML/OCR (Optional)
- `candle-*` - Neural network inference
- `tokenizers` - Text tokenization for ML models
- `ocrs` - Pure Rust OCR engine
- `rten` - Neural network runtime for OCR

### Utilities
- `nucleo` - Fuzzy file finder
- `lru` - Caching for two-pass system
- `notify` - File watching for auto-reload

## Critical Issues (v7.64.0)
1. Text extraction produces garbled/incorrect output
2. Clipboard commands don't work properly
3. Selection/cursor positioning is broken
4. Performance degradation with ML features
5. Screen jittering during updates

## Key Design Decisions
1. **No Ratatui** - Direct crossterm to avoid screen tearing
2. **Character Grid** - Fixed-size grid (typically 200×60) for spatial layout
3. **Two-Pass Caching** - Expensive ML operations cached between renders
4. **Spatial Preservation** - Maintains PDF layout in terminal grid

## File Structure
```
src/
├── main.rs              # App state, event loop
├── content_extractor.rs # PDFium text extraction
├── two_pass.rs         # Caching layer
├── edit_renderer.rs    # Text editing UI
├── pdf_renderer.rs     # PDF to image
├── ml/                 # ML models (optional)
└── ocr/                # OCR support (optional)
```

## Binary Requirements
- Requires `DYLD_LIBRARY_PATH` set to find `libpdfium.dylib`
- Wrapper script at `~/.local/bin/chonker7` sets library path
- Actual binary at `~/.local/bin/chonker7-bin`