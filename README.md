# Chonker7

A terminal PDF viewer that combines **fancy-cat** inspired terminal display with **Ferrules** AI-powered text extraction into a spatial text matrix.

## ✨ Features

- 🤖 **AI-Powered Text Extraction** - Uses Ferrules for intelligent document parsing
- 📊 **Text Matrix Display** - Preserves spatial layout of extracted text
- 🖼️ **Split View** - Side-by-side PDF image and EDIT panel
- ⚡ **Fast Navigation** - Quick page switching with keyboard shortcuts
- 🔄 **Multiple Display Modes** - PDF+EDIT, PDF+MARKDOWN, or OPTIONS
- 🚀 **Global Command** - Run from anywhere with `chonker7`

## Concept

Chonker7 bridges the gap between visual PDF display and intelligent text extraction by:
1. Using fancy-cat's approach for PDF image display in terminal
2. Leveraging ferrules for AI-powered text extraction
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

- **Simplicity**: Pure Rust implementation, easy to build and extend
- **Intelligence**: Ferrules AI understands document structure semantically
- **Terminal-First**: Designed for terminal workflows
- **Spatial Preservation**: Text matrix maintains document layout
- **Fast**: Instant page navigation and mode switching

## 🛠️ Technical Details

- **Language**: Rust
- **PDF Extraction**: Ferrules (AI-powered document parser)
- **Terminal UI**: Ratatui + Crossterm
- **Image Display**: Kitty graphics protocol support (planned)
- **Text Layout**: Custom matrix renderer preserving spatial relationships

## 📝 License

MIT