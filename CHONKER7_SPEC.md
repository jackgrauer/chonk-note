# Chonker7 v7.64.0 - Terminal PDF Viewer/Editor Specification

## ⚠️ CRITICAL ARCHITECTURAL CLARIFICATION

**Chonker7 uses PURE PDFium for PDF processing, NOT Ferrules.**

Despite the presence of `/ferrules` directory in the codebase:
- All PDF text extraction uses `pdfium-render` v0.8 directly
- The ferrules directory is **dormant/reference code only**
- No imports from ferrules in any active source files
- Ships with `libpdfium.dylib` in `/lib/` for direct PDFium access
- ML features mentioned below are **stubbed/removed** in current implementation

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
- `content_extractor.rs` - Main PDF text extraction using **pure PDFium** (NOT ferrules)
- `simple_pdfium_extractor.rs` - Alternative PDFium extractor with basic table detection
- `edit_renderer.rs` - Text editing UI with cursor, selection, clipboard
- `pdf_renderer.rs` - PDF page rendering to images via PDFium
- `viuer_display.rs` - Terminal image display (fallback when text fails)
- `two_pass.rs` - **REMOVED** (was for ML caching)
- `markdown_renderer.rs` - **REMOVED** in current version

### ML Features (REMOVED/STUBBED)
- `ml/` - **NOT PRESENT** - No ML models in use
- `ocr/` - **NOT ACTIVE** - No OCR functionality
- `/ferrules` directory - **DORMANT** - Not imported or used
- All ML-related functions return empty grids or stub responses

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

### ML/OCR (NOT USED)
- ~~`candle-*`~~ - Not in dependencies
- ~~`tokenizers`~~ - Not in dependencies  
- ~~`ocrs`~~ - Not in dependencies
- ~~`rten`~~ - Not in dependencies
- ~~`ort`~~ - Not needed (avoids v1→v2 breaking changes)

### Utilities
- `nucleo` - Fuzzy file finder
- `lru` - Caching for two-pass system
- `notify` - File watching for auto-reload

## Critical Issues (v7.64.0)
1. Text extraction produces garbled/incorrect output
2. Clipboard commands don't work properly
3. Selection/cursor positioning is broken
4. ~~Performance degradation with ML features~~ - RESOLVED by removing ML
5. Screen jittering during updates

## PDFium vs Ferrules Trade-offs

| Aspect | Current (PDFium) | Ferrules (if integrated) |
|--------|------------------|-------------------------|
| Startup | <100ms | 2-5 seconds (model load) |
| Memory | ~50MB | 500MB+ |
| Accuracy | Basic spatial | Advanced semantic |
| Tables | No detection | ML-powered detection |
| Dependencies | Just libpdfium.dylib | ONNX runtime + models |
| Maintenance | Simple | Complex (ort version issues) |

## Key Design Decisions
1. **Pure PDFium over Ferrules** - Chose simplicity/speed over ML accuracy
2. **No Ratatui** - Direct crossterm to avoid screen tearing
3. **Character Grid** - Fixed-size grid (typically 200×100) for spatial layout
4. **No ML/Caching** - Removed two-pass system, instant page loads
5. **Spatial Preservation** - Direct coordinate mapping from PDF to grid

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