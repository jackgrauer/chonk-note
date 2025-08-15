# Integration Guide: PDFium EDIT Tab Overhaul

## Quick Start

To integrate the new PDFium-based extraction into chonker7:

### 1. Add the new module to main.rs
```rust
// Add after existing mod declarations
mod pdfium_spatial_extractor;
mod content_extractor_pdfium;  // Enhanced version

// Optional: alias for testing
#[cfg(feature = "pdfium_enhanced")]
use content_extractor_pdfium as content_extractor;
```

### 2. Update Cargo.toml (if needed)
The current pdfium-render v0.8 should work, but consider updating:
```toml
pdfium-render = { version = "0.8.31", features = ["thread_safe", "image"] }
```

### 3. Test with Simple Integration
```rust
// In keyboard.rs or main.rs, modify the extract handler:
KeyCode::Char('e') if modifiers == MOD_KEY => {
    app.status_message = "Extracting with PDFium spatial analysis...".to_string();
    
    // Use new extractor
    runtime.block_on(content_extractor_pdfium::extract_to_matrix(
        &app.pdf_path,
        app.current_page,
        matrix_width,
        matrix_height,
    ))?;
}
```

## Key Improvements Over Current Implementation

### Current (Ferrules-based)
- Block-level text extraction
- ML-based layout detection  
- Limited spatial awareness
- No character-level coordinates
- Poor table handling

### New (PDFium-based)
- **Character-level precision**: Every character's exact position
- **Font metadata**: Size, weight, style, color for each character
- **Spatial clustering**: Docstrum-like algorithm for word/line/paragraph detection
- **Table detection**: Coordinate alignment analysis
- **Rotation handling**: Proper support for rotated text
- **RTL support**: Detection and handling of right-to-left languages

## Performance Considerations

### Memory Usage
- Character extraction: ~100 bytes per character
- For 10,000 char page: ~1MB
- Use lazy loading for large documents

### Speed Optimizations
- Batch character extraction per text object
- R-tree spatial indexing for fast queries  
- Cache extracted data per page
- Progressive rendering for large pages

## Testing Checklist

### Basic Functionality
- [ ] Text extraction works
- [ ] Coordinates are accurate
- [ ] Font metadata extracted
- [ ] Grid mapping preserves layout

### Advanced Features  
- [ ] Tables detected and rendered
- [ ] Headers identified by font size
- [ ] Code blocks (monospace) detected
- [ ] Italic text preserved
- [ ] Multi-column layouts handled

### Edge Cases
- [ ] Rotated text handled
- [ ] RTL languages work
- [ ] Large documents (100+ pages)
- [ ] Dense text (10,000+ chars/page)
- [ ] Mixed content (text + tables)

## Debugging Tips

### Enable Coordinate Visualization
```rust
// Add to renderer.rs for debugging
if DEBUG_COORDS {
    for char in &characters {
        let (x, y) = coordinates::to_grid_position(...);
        grid[y][x] = '█';  // Show character positions
    }
}
```

### Log Character Metadata
```rust
// In pdfium_spatial_extractor.rs
println!("Char: {} @ ({:.1}, {:.1}) size:{:.1} weight:{}", 
    char.unicode, 
    char.page_position.0, 
    char.page_position.1,
    char.font_size,
    char.font_weight
);
```

### Validate Clustering
```rust
// Check word clustering accuracy
for word in &word_clusters {
    let text: String = word.iter().map(|c| c.unicode).collect();
    println!("Word: '{}'", text);
}
```

## Common Issues & Solutions

### Issue: Text overlapping in grid
**Solution**: Check coordinate transformation and grid mapping calculations. Ensure proper screen space conversion.

### Issue: Tables not detected
**Solution**: Adjust tolerance parameters in `TableDetector`. Check if minimum column/row thresholds are too high.

### Issue: Wrong reading order
**Solution**: Verify line sorting by y-coordinate. Check if using screen space (top-to-bottom) vs PDF space (bottom-to-top).

### Issue: Memory spike on large PDFs
**Solution**: Implement page-level caching with LRU eviction. Process pages on-demand rather than pre-loading.

## Migration Path

### Phase 1: Parallel Testing
1. Keep both extractors available
2. Add feature flag to switch between them
3. Compare outputs on test corpus

### Phase 2: Gradual Rollout
1. Use PDFium for simple PDFs first
2. Fall back to Ferrules for complex layouts
3. Gather performance metrics

### Phase 3: Full Migration
1. Remove Ferrules dependency
2. Optimize PDFium extraction
3. Add advanced features (tables, etc.)

## Benchmarks to Track

```rust
// Add to test.rs
#[bench]
fn bench_extract_page() {
    // Measure: 
    // - Time to extract 1 page
    // - Memory allocated
    // - Character accuracy
    // - Table detection rate
}
```

Target metrics:
- Extract page: <200ms
- Memory per page: <2MB  
- Character accuracy: >99%
- Table detection: >90% F1

## Future Enhancements

### Near-term (1-2 weeks)
- [ ] Implement full Docstrum algorithm
- [ ] Add table cell content extraction
- [ ] Support inline images/figures
- [ ] Handle footnotes properly

### Medium-term (1 month)
- [ ] OCR integration for scanned PDFs
- [ ] Advanced column detection
- [ ] Mathematical equation handling
- [ ] Form field extraction

### Long-term
- [ ] Machine learning for layout understanding
- [ ] Multi-page flow analysis
- [ ] Semantic structure extraction
- [ ] Export to other formats (DOCX, HTML)

## Resources

- [PDFium C++ API](https://pdfium.googlesource.com/pdfium/+/master/public/)
- [pdfium-render Rust docs](https://docs.rs/pdfium-render/)
- [Docstrum paper](https://www.researchgate.net/publication/220932903_The_Document_Spectrum_for_Page_Layout_Analysis)
- [Table detection algorithms](https://github.com/camelot-dev/camelot)
