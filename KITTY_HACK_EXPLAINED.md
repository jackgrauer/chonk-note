# How We Hacked the Kitty Image Protocol to Display PDFs

## The Core Hack: PDF → Image → Terminal

The "hack" is essentially bypassing complex text extraction and just displaying the PDF page as an image directly in the terminal. This gives you a "kinda readable" PDF because you can see the actual rendered page, but you can't select/copy text.

## The Pipeline

```
1. PDF File (on disk)
   ↓
2. PDFium renders page to bitmap
   ↓  
3. Convert to image::DynamicImage
   ↓
4. Viuer library auto-detects terminal capabilities
   ↓
5. Terminal displays image inline
```

## Key Components

### 1. PDF Rendering (`pdf_renderer.rs`)
```rust
// Use PDFium to render PDF page to an image at specific resolution
render_pdf_page(pdf_path, page_num, width, height)
→ Loads PDF with PDFium
→ Calculates scale to fit terminal dimensions  
→ Renders to bitmap maintaining aspect ratio
→ Returns DynamicImage
```

### 2. Image Display (`viuer_display.rs`)
```rust
display_pdf_image(image, x, y, max_width, max_height, dark_mode)
→ Converts image from v0.25 to v0.24 (compatibility hack!)
→ Optionally inverts colors for dark mode
→ Uses viuer with these key settings:
   - use_kitty: true  (Try Kitty protocol first)
   - use_iterm: true  (Fallback to iTerm2)
   - transparent: true (For PDFs with transparency)
   - absolute_offset: true (Position at x,y)
```

### 3. The Viuer Magic
Viuer automatically detects and uses the best available protocol:
1. **Kitty Graphics Protocol** - If `KITTY_WINDOW_ID` env var exists
2. **iTerm2 Inline Images** - If `TERM_PROGRAM=iTerm.app`
3. **Sixel** - If terminal supports it
4. **Block characters** - Unicode block art fallback (very pixelated)

## The Kitty Protocol Details

When Kitty is detected, viuer sends:
- Base64-encoded PNG data
- Escape sequences: `\x1b_G` (graphics command)
- Positioning and sizing parameters
- The terminal renders it inline with text

## Why It's "Kinda Readable"

### Pros:
- You see the actual PDF with proper fonts, formatting, images
- Works with any PDF (scanned, complex layouts, etc.)
- No text extraction errors
- Preserves exact visual appearance

### Cons:
- Can't select or copy text
- Resolution limited by terminal dimensions
- Scrolling is page-by-page, not smooth
- Large memory usage (entire page as image)
- Text can be fuzzy depending on terminal DPI

## The Clever Hacks

1. **Version Compatibility Hack**: Convert between incompatible image crate versions (0.25 → 0.24) because viuer uses old version

2. **Dark Mode Hack**: Simply invert RGB values pixel-by-pixel
   ```rust
   pixel[0] = 255 - pixel[0]; // Invert R
   pixel[1] = 255 - pixel[1]; // Invert G  
   pixel[2] = 255 - pixel[2]; // Invert B
   ```

3. **Clear Graphics Hack**: Send raw escape sequences to clear images
   ```rust
   // Kitty: \x1b_Ga=d\x1b\\
   // iTerm2: \x1b]1337;File=inline=0:\x07
   ```

4. **Fallback Chain**: If image display fails, try text extraction. If that fails too, show error message.

## Why This Approach?

The complex text extraction code was producing garbled output, but PDFs rendered as images always look correct (even if fuzzy). This was meant as a fallback but ended up being more reliable than the text extraction for many PDFs.

The irony: We built a sophisticated ML-powered text extraction system, but often the simple "just show it as an image" approach works better for reading PDFs in the terminal!