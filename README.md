# Chonker7

⚠️ **IMPORTANT ARCHITECTURAL NOTE**: Despite the presence of a `/ferrules` directory, Chonker7 does **NOT** use Ferrules for PDF processing. It uses **pure PDFium** directly via `pdfium-render` crate for all text extraction. The Ferrules directory is dormant/reference code only.

A terminal PDF viewer that combines **fancy-cat** inspired terminal display with **direct PDFium** text extraction into a spatial text matrix.

## ✨ Features

- 📄 **Direct PDFium Text Extraction** - Pure PDFium without ML overhead (NOT Ferrules)
- 📊 **Text Matrix Display** - Preserves spatial layout of extracted text
- 🖼️ **Split View** - Side-by-side PDF image and EDIT panel
- ⚡ **Fast Navigation** - Quick page switching with keyboard shortcuts
- 🔄 **Multiple Display Modes** - PDF+EDIT, PDF+MARKDOWN, or OPTIONS
- 🚀 **Global Command** - Run from anywhere with `chonker7`

## Concept

Chonker7 bridges the gap between visual PDF display and text extraction by:
1. Using fancy-cat's approach for PDF image display in terminal
2. **Using direct PDFium bindings** for lightweight text extraction (no Ferrules/ML)
3. Presenting extracted text in a preserved spatial matrix layout

## Architecture

```
┌─────────────────────────────────────┐
│          Chonker7 CLI               │
├─────────────────────────────────────┤
│                                     │
│  ┌─────────────┐  ┌──────────────┐ │
│  │   PDF View  │  │   EDIT Panel │ │
│  │  (Image)    │  │  (Gridlike)  │ │
│  └─────────────┘  └──────────────┘ │
│                                     │
│  ┌─────────────────────────────────┐│
│  │       Terminal Display          ││
│  │    (Kitty Image Protocol)       ││
│  └─────────────────────────────────┘│
└─────────────────────────────────────┘
```

## 📦 Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/chonker7.git
cd chonker7

# Install as global command
./install.sh
```

## 🚀 Usage

```bash
# Open with file dialog (macOS native)
chonker7

# Open specific PDF
chonker7 document.pdf

# Start at specific page
chonker7 document.pdf -p 5

# OPTIONS mode
chonker7 document.pdf -m options

# MARKDOWN view  
chonker7 document.pdf -m markdown

# EDIT view (default)
chonker7 document.pdf -m edit
```

## ⌨️ Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+O` | Open new PDF (file dialog) |
| `Ctrl+N` / `→` | Next page |
| `Ctrl+P` / `←` | Previous page |
| `Tab` | Toggle display mode (PDF+EDIT → PDF+MARKDOWN → OPTIONS) |
| `Ctrl+D` | Toggle dark/light mode |
| `Ctrl+E` | Re-extract current page |
| `Ctrl+Q` | Quit |

## 🎯 Why Chonker7?

- **Simplicity**: Pure PDFium implementation without ML complexity
- **Lightweight**: No ONNX runtime, no model loading, instant startup
- **Terminal-First**: Designed for terminal workflows
- **Spatial Preservation**: Text matrix maintains document layout
- **Fast**: No ML inference overhead = instant page navigation

## 📋 PDFium vs Ferrules Decision

| Aspect | Chonker7 (PDFium) | Ferrules |
|--------|-------------------|----------|
| PDF Library | Direct PDFium bindings | PDFium + ML wrapper |
| ML/Layout Detection | None | LayoutLM v3, YOLO |
| Dependencies | Minimal | Heavy (ort, candle) |
| Startup Time | Instant | Model loading delay |
| Memory Usage | ~50MB | ~500MB+ |
| Text Accuracy | Basic | Advanced with entity recognition |
| Table Detection | None | ML-powered |

**Why PDFium only?** Chonker7 prioritizes speed and simplicity for terminal editing over advanced document understanding.

## 🛠️ Technical Details

- **Language**: Rust
- **PDF Extraction**: Pure PDFium via `pdfium-render` v0.8 (NOT Ferrules)
  - Ships with `libpdfium.dylib` in `/lib/` directory
  - No ML/AI models, no ONNX runtime dependencies
  - Direct character-to-grid spatial mapping
- **Terminal UI**: Crossterm only (no Ratatui to avoid tearing)
- **Image Display**: Viuer for fallback display
- **Text Layout**: Custom 200×100 character grid preserving spatial relationships

## 📝 License

MIT