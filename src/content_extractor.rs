// CHONKER 7.34 - TEXT EXTRACTION KNOWLEDGE BASE
// ==============================================
//
// After extensive iteration, we have two distinct approaches:
//
// 1. COLUMN-AWARE APPROACH (v7.28-v7.34) - BEST FOR TABLES
//    - Uses TextLine structures to group by baseline
//    - Detects column boundaries via consistent gaps
//    - ColumnAwareGridMapper splits content into columns
//    - Adds pipe separators between columns
//    - PROS: Excellent for tables, preserves column structure
//    - CONS: Can struggle with regular text, may add unwanted spacing
//
// 2. SIMPLE SEQUENTIAL APPROACH (v7.27) - BEST FOR TEXT
//    - Places words sequentially left-to-right
//    - Simple word clustering based on gaps
//    - No column detection logic
//    - PROS: Works well for regular text paragraphs
//    - CONS: Tables become unreadable blobs of concatenated values
//
// CURRENT IMPLEMENTATION: Column-aware (better for mixed content)
// Trade-off: Some regular text may have odd spacing, but tables are readable
//
// Key parameters that work:
// - Grid size: 400x200 (captures full tables)
// - Column detection threshold: 60% of lines
// - Gap threshold: 0.5 * font_size
// - Row merge tolerance: 2.0 pixels

use anyhow::Result;
use pdfium_render::prelude::*;
use std::path::Path;
use std::collections::HashMap;
use ordered_float::OrderedFloat;

/// Complete page data for ML processing
#[derive(Debug, Clone)]
pub struct PageData {
    pub characters: Vec<CharacterData>,
    pub page_width: f32,
    pub page_height: f32,
    pub page_image: Option<image::DynamicImage>,
    pub annotations: Vec<AnnotationData>,
    pub form_fields: Vec<FormFieldData>,
}

/// Character data extracted from PDFium with full spatial information
#[derive(Debug, Clone)]
pub struct CharacterData {
    pub unicode: char,
    pub x: f32,           // Position in PDF coordinates
    pub y: f32,
    pub width: f32,       // Character width
    pub height: f32,      // Character height
    pub font_size: f32,
    pub baseline_y: f32,  // Text baseline for alignment
    // New PDFium properties
    pub font_name: Option<String>,     // Font family name
    pub font_weight: u32,              // Font weight (400=normal, 700=bold)
    pub is_italic: bool,               // Font style
    pub is_monospace: bool,            // Fixed-width font
    pub scaled_font_size: f32,         // Actual rendered font size
    pub is_generated: bool,            // PDFium generated this char
    pub is_hyphen: bool,               // Character is a hyphen
    pub char_angle: f32,               // Rotation angle in radians
}

/// Represents a text line with grouped characters
#[derive(Debug, Clone)]
struct TextLine {
    baseline: f32,
    chars: Vec<CharacterData>,
    x_start: f32,
    x_end: f32,
}

/// Represents a detected table structure
#[derive(Debug)]
struct TableStructure {
    rows: Vec<f32>,     // Y coordinates of row boundaries
    columns: Vec<f32>,  // X coordinates of column boundaries
    cells: Vec<Vec<String>>, // Cell contents
}

/// Annotation data from PDF
#[derive(Debug, Clone)]
struct AnnotationData {
    annotation_type: String,  // Highlight, Note, Link, etc.
    bounds: (f32, f32, f32, f32), // (left, top, right, bottom)
    contents: Option<String>, // Annotation text content
    author: Option<String>,   // Creator of annotation
    color: Option<(u8, u8, u8)>, // RGB color
}

/// Form field data from PDF
#[derive(Debug, Clone)]
struct FormFieldData {
    field_type: String,       // Text, Checkbox, Radio, etc.
    name: String,             // Field name
    value: Option<String>,    // Current value
    bounds: (f32, f32, f32, f32), // (left, top, right, bottom)
    is_readonly: bool,
    is_required: bool,
}

/// Simple extraction strategy - just table or text
#[derive(Debug, Clone)]
enum ExtractionStrategy {
    Table,  // Has grid lines or strong column alignment
    Text,   // Everything else - preserve natural spacing
}

/// Column-aware grid mapper for preserving table structure
struct ColumnAwareGridMapper {
    columns: Vec<f32>,      // X positions of column boundaries
    col_widths: Vec<usize>, // Grid cells per column
}

impl ColumnAwareGridMapper {
    fn new(columns: Vec<f32>, total_width: usize) -> Self {
        // Calculate grid width for each column
        let num_cols = columns.len() + 1;
        let base_width = total_width / num_cols;
        let col_widths = vec![base_width; num_cols];
        
        Self {
            columns,
            col_widths,
        }
    }
    
    fn map_to_grid(&self, lines: &[TextLine], width: usize, height: usize) -> Vec<Vec<char>> {
        let mut grid = vec![vec![' '; width]; height];
        
        for (y, line) in lines.iter().enumerate() {
            if y >= height { break; }
            
            // Split line into columns
            let cells = self.split_line_by_columns(line);
            
            let mut grid_x = 0;
            for (col_idx, cell_text) in cells.iter().enumerate() {
                let col_width = self.col_widths.get(col_idx).copied().unwrap_or(10);
                
                // Detect if content is numeric (for right-alignment)
                let is_numeric = cell_text.chars().any(|c| c == '$' || c.is_ascii_digit()) &&
                                !cell_text.chars().any(|c| c.is_alphabetic() && c != 'N' && c != 'A');
                
                if is_numeric && cell_text.len() > 0 {
                    // Right align numbers
                    let start = grid_x + col_width.saturating_sub(cell_text.len()).saturating_sub(1);
                    for (i, ch) in cell_text.chars().enumerate() {
                        if start + i < width {
                            grid[y][start + i] = ch;
                        }
                    }
                } else {
                    // Left align text
                    for (i, ch) in cell_text.chars().take(col_width).enumerate() {
                        if grid_x + i < width {
                            grid[y][grid_x + i] = ch;
                        }
                    }
                }
                
                grid_x += col_width;
                if grid_x < width && col_idx < cells.len() - 1 {
                    // Add a small gap between columns
                    grid_x += 1; // Space before separator
                    if grid_x < width {
                        grid[y][grid_x] = '|'; // Column separator
                    }
                    grid_x += 2; // Space after separator
                }
            }
        }
        
        grid
    }
    
    fn split_line_by_columns(&self, line: &TextLine) -> Vec<String> {
        let mut cells = vec![String::new(); self.columns.len() + 1];
        
        for ch in &line.chars {
            // Find which column this character belongs to
            let col_idx = self.columns.iter()
                .position(|&col_x| ch.x < col_x)
                .unwrap_or(self.columns.len());
            
            cells[col_idx].push(ch.unicode);
        }
        
        // Trim whitespace from cells and ensure spacing
        for cell in cells.iter_mut() {
            *cell = cell.trim().to_string();
            // Add a space at the end to prevent column merging
            if !cell.is_empty() {
                cell.push(' ');
            }
        }
        
        cells
    }
}

/// Simple detection - just check if page has table indicators
fn detect_extraction_strategy(page: &PdfPage) -> ExtractionStrategy {
    let mut line_count = 0;
    let mut has_horizontal_lines = false;
    let mut has_vertical_lines = false;
    
    // Count path objects (lines) that look like table borders
    for object in page.objects().iter() {
        if let Some(path) = object.as_path_object() {
            if let Ok(bounds) = path.bounds() {
                // Horizontal line?
                if bounds.height().value < 2.0 && bounds.width().value > 50.0 {
                    has_horizontal_lines = true;
                    line_count += 1;
                }
                // Vertical line?
                if bounds.width().value < 2.0 && bounds.height().value > 20.0 {
                    has_vertical_lines = true;
                    line_count += 1;
                }
            }
        }
    }
    
    // If we see both horizontal and vertical lines, it's likely a table
    if has_horizontal_lines && has_vertical_lines && line_count >= 4 {
        eprintln!("Detected TABLE page ({} grid lines found)", line_count);
        ExtractionStrategy::Table
    } else {
        eprintln!("Detected TEXT page (no table grid found)");
        ExtractionStrategy::Text
    }
}

/// Extract annotations from PDF page
fn extract_annotations(page: &PdfPage) -> Vec<AnnotationData> {
    let mut annotations = Vec::new();
    
    for annot in page.annotations().iter() {
        if let Ok(bounds) = annot.bounds() {
            let annotation_type = format!("{:?}", annot.annotation_type());
            let contents = annot.contents();
            
            annotations.push(AnnotationData {
                annotation_type,
                bounds: (
                    bounds.left().value,
                    bounds.top().value,
                    bounds.right().value,
                    bounds.bottom().value,
                ),
                contents,
                author: None, // Author not available in current pdfium-render
                color: None, // TODO: extract color if available
            });
        }
    }
    
    annotations
}

/// Extract form fields from PDF page
fn extract_form_fields(_page: &PdfPage) -> Vec<FormFieldData> {
    let fields = Vec::new();
    
    // PDFium form field extraction would go here
    // Note: This requires the form API which may not be fully exposed
    // through pdfium-render yet
    
    fields
}

pub fn get_page_count(pdf_path: &Path) -> Result<usize> {
    // Use our pdf_renderer module which already has PDFium integration
    crate::pdf_renderer::get_pdf_page_count(pdf_path)
}

// Safety bounds to prevent resource exhaustion
const MAX_CHARS_PER_PAGE: usize = 50_000;
const MAX_GRID_SIZE: usize = 1_000_000; // max width * height

// Table detection constants
const COLUMN_GAP_THRESHOLD: f32 = 0.5;  // * font_size
const ROW_MERGE_TOLERANCE: f32 = 2.0;   // pixels
const MIN_TABLE_ROWS: usize = 2;
const MIN_TABLE_COLS: usize = 2;

pub async fn extract_to_matrix(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Vec<char>>> {
    // Safety check: Prevent excessive memory allocation
    if width * height > MAX_GRID_SIZE {
        return Err(anyhow::anyhow!(
            "Grid size {}x{} exceeds maximum allowed size",
            width, height
        ));
    }
    
    // Initialize grid
    let mut grid = vec![vec![' '; width]; height];
    
    // Use PDFium for character-level extraction
    let pdfium = crate::pdf_renderer::get_pdfium_instance();
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;
    
    // Get page properties
    let page_width = page.width().value;
    let page_height = page.height().value;
    
    // Show page info in stderr for debugging
    eprintln!("Page properties: {:.0}x{:.0} pts", page_width, page_height);
    
    // Extract characters with spatial data
    let characters = extract_characters_from_page(&page)?;
    
    // Extract annotations and form fields
    let annotations = extract_annotations(&page);
    let _form_fields = extract_form_fields(&page);
    
    // Show debug info about extracted properties
    if !annotations.is_empty() {
        eprintln!("Found {} annotations on page", annotations.len());
        for annot in &annotations {
            if let Some(ref contents) = annot.contents {
                eprintln!("  - {}: {}", annot.annotation_type, contents);
            }
        }
    }
    
    // Show font info for first few characters to demonstrate extraction
    if !characters.is_empty() && characters.len() <= 10 {
        eprintln!("Font properties for first characters:");
        for (i, ch) in characters.iter().take(5).enumerate() {
            eprintln!("  Char {}: '{}' font={:?} size={:.1} weight={} italic={}", 
                i, ch.unicode, ch.font_name, ch.scaled_font_size, ch.font_weight, ch.is_italic);
        }
    }
    
    // Safety check: Prevent processing too many characters
    if characters.len() > MAX_CHARS_PER_PAGE {
        // Fall back to simple extraction for complex pages
        return Ok(simple_text_fallback(&characters, width, height));
    }
    
    // SIMPLIFIED EXTRACTION: Just detect table vs text for the whole page
    let strategy = detect_extraction_strategy(&page);
    let text_lines = build_text_lines(&characters);
    
    // Check for images and add placeholders
    let mut current_grid_y = 0;
    let mut has_images = false;
    
    // Add page info header
    let header = format!("[Page: {:.0}x{:.0}pts]", page_width, page_height);
    
    for (x, ch) in header.chars().enumerate() {
        if x < width && current_grid_y < height {
            grid[current_grid_y][x] = ch;
        }
    }
    current_grid_y += 2; // Space after header
    
    // First, add image placeholders if any
    for object in page.objects().iter() {
        if object.object_type() == PdfPageObjectType::Image {
            if let Ok(_bounds) = object.bounds() {
                // Add image placeholder at appropriate position
                if !has_images {
                    let image_text = "[Image/Figure]";
                    for (x, ch) in image_text.chars().enumerate() {
                        if x < width && current_grid_y < height {
                            grid[current_grid_y][x] = ch;
                        }
                    }
                    current_grid_y += 2; // Space after image
                    has_images = true;
                }
            }
        }
    }
    
    // Add annotation markers if any
    if !annotations.is_empty() {
        let annot_text = format!("[{} Annotations]", annotations.len());
        for (x, ch) in annot_text.chars().enumerate() {
            if x < width && current_grid_y < height {
                grid[current_grid_y][x] = ch;
            }
        }
        current_grid_y += 2;
    }
    
    match strategy {
        ExtractionStrategy::Table => {
            // Use column-aware extraction for tables
            let columns = detect_column_boundaries(&text_lines);
            if !columns.is_empty() && columns.len() >= 2 {
                let mapper = ColumnAwareGridMapper::new(columns, width);
                let table_grid = mapper.map_to_grid(&text_lines, width, height - current_grid_y);
                
                // Copy table to main grid
                for (y, row) in table_grid.iter().enumerate() {
                    if current_grid_y + y < height {
                        for (x, &ch) in row.iter().enumerate() {
                            if x < width && ch != ' ' {
                                grid[current_grid_y + y][x] = ch;
                            }
                        }
                    }
                }
            } else {
                // No columns detected, fall back to simple mapping
                map_lines_to_grid_with_offset(&mut grid, &text_lines, width, height, current_grid_y);
            }
        }
        ExtractionStrategy::Text => {
            // Use natural spacing for text
            map_lines_to_grid_with_natural_spacing(&mut grid, &text_lines, width, height, current_grid_y);
        }
    }
    
    Ok(grid)
}

/// Simple fallback for complex pages - just dump text without fancy layout
fn simple_text_fallback(characters: &[CharacterData], width: usize, height: usize) -> Vec<Vec<char>> {
    let mut grid = vec![vec![' '; width]; height];
    
    // Just place characters in simple top-to-bottom, left-to-right order
    let mut row = 0;
    let mut col = 0;
    
    for ch in characters {
        if col >= width {
            col = 0;
            row += 1;
        }
        if row >= height {
            break;
        }
        
        grid[row][col] = ch.unicode;
        col += 1;
    }
    
    grid
}

/// Extract individual characters from a PDF page with spatial data
fn extract_characters_from_page(page: &PdfPage) -> Result<Vec<CharacterData>> {
    let mut characters = Vec::new();
    let text_page = page.text()?;
    
    // Get page dimensions for coordinate normalization
    let page_height = page.height().value;
    
    // Extract characters using the chars() method which returns a collection
    for char in text_page.chars().iter() {
        // P0.1 FIX: PDFium can return multi-char strings (ligatures, etc)
        // Use unicode_string() instead of unicode_char() to handle all cases
        let unicode_string = match char.unicode_string() {
            Some(s) => s,
            None => continue, // Skip if no unicode string
        };
        let unicode = match unicode_string.chars().next() {
            Some(c) => c,
            None => continue, // Skip if no unicode character
        };
        
        // Skip non-printable characters
        if unicode.is_control() || unicode == '\0' {
            continue;
        }
        
        let bounds = char.loose_bounds()?;
        
        // Get font information if available
        // Note: Many of these methods aren't exposed in pdfium-render 0.8
        // We'll use what's available and provide defaults for the rest
        let font_name = None; // Not available in current API
        let font_weight = 400; // Default to normal weight
        let is_italic = false; // Would need font flags
        let is_monospace = false; // Would need font flags
        
        // Use character height as font size estimate
        let scaled_font_size = bounds.height().value;
        
        // Check if character is a hyphen
        let is_generated = false; // Not available in current API
        let is_hyphen = unicode == '-' || unicode == '­';  // Regular or soft hyphen
        
        // Get character rotation angle - also not available on text chars
        let char_angle = 0.0; // Default to no rotation
        
        // Estimate display font size from character height if scaled size not available
        let font_size = if scaled_font_size > 0.0 {
            scaled_font_size
        } else {
            bounds.height().value
        };
        
        // No rotation transformation for now - pdfium-render doesn't expose rotation easily
        let x = bounds.left().value;
        let y = page_height - bounds.top().value; // Convert to top-down coordinates
        let baseline_y = page_height - bounds.bottom().value;
        
        characters.push(CharacterData {
            unicode,
            x,
            y,
            width: bounds.width().value,
            height: bounds.height().value,
            font_size,
            baseline_y,
            // New properties
            font_name,
            font_weight,
            is_italic,
            is_monospace,
            scaled_font_size,
            is_generated,
            is_hyphen,
            char_angle,
        });
    }
    
    Ok(characters)
}

/// Build text lines with proper baseline grouping
fn build_text_lines(chars: &[CharacterData]) -> Vec<TextLine> {
    if chars.is_empty() {
        return Vec::new();
    }
    
    // Group by baseline with tolerance
    let mut lines_map: HashMap<OrderedFloat<f32>, Vec<CharacterData>> = HashMap::new();
    
    for ch in chars {
        // Round baseline to nearest 2 pixels for grouping
        let baseline_key = OrderedFloat((ch.baseline_y / ROW_MERGE_TOLERANCE).round() * ROW_MERGE_TOLERANCE);
        lines_map.entry(baseline_key).or_default().push(ch.clone());
    }
    
    // Convert to TextLine structures
    let mut lines: Vec<TextLine> = lines_map.into_iter()
        .map(|(baseline, mut chars)| {
            // Sort chars within each line by X position
            chars.sort_by_key(|c| OrderedFloat(c.x));
            
            let x_start = chars.first().map(|c| c.x).unwrap_or(0.0);
            let x_end = chars.last().map(|c| c.x + c.width).unwrap_or(0.0);
            
            TextLine {
                baseline: baseline.0,
                chars,
                x_start,
                x_end,
            }
        })
        .collect();
    
    // Sort lines by baseline (top to bottom)
    lines.sort_by_key(|l| OrderedFloat(l.baseline));
    
    lines
}

/// Group characters by baseline with tolerance (legacy function for compatibility)
fn group_by_baseline(characters: &[CharacterData]) -> Vec<Vec<CharacterData>> {
    if characters.is_empty() {
        return Vec::new();
    }
    
    let mut lines: Vec<Vec<CharacterData>> = Vec::new();
    let mut sorted_chars = characters.to_vec();
    
    // P0.2 FIX: Group by baseline first to preserve columns
    // Safe sort that won't panic on NaN
    sorted_chars.sort_by(|a, b| {
        use std::cmp::Ordering;
        a.baseline_y.partial_cmp(&b.baseline_y).unwrap_or(Ordering::Equal)
    });
    
    let mut current_line = Vec::new();
    let mut current_baseline = sorted_chars[0].baseline_y;
    
    for char in sorted_chars {
        // Use font-size based tolerance for baseline grouping
        let tolerance = char.font_size * 0.3;
        
        if (char.baseline_y - current_baseline).abs() > tolerance {
            // New line detected
            if !current_line.is_empty() {
                // Sort characters within the line by x position
                current_line.sort_by(|a: &CharacterData, b: &CharacterData| {
                    a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                });
                lines.push(current_line);
                current_line = Vec::new();
            }
            current_baseline = char.baseline_y;
        }
        
        current_line.push(char);
    }
    
    // Don't forget the last line
    if !current_line.is_empty() {
        current_line.sort_by(|a: &CharacterData, b: &CharacterData| {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        });
        lines.push(current_line);
    }
    
    lines
}

/// Cluster characters into words based on spatial proximity
fn cluster_into_words(characters: &[CharacterData]) -> Vec<Vec<CharacterData>> {
    let mut words = Vec::new();
    
    // P0.2 FIX: Group by baseline first to preserve column structure
    let lines = group_by_baseline(characters);
    
    // Process each line to extract words
    for line in lines {
        let mut current_word = Vec::new();
        let mut last_char: Option<&CharacterData> = None;
        
        for char_data in &line {
            if let Some(last) = last_char {
                // Check if this character is part of the same word
                let horizontal_gap = char_data.x - (last.x + last.width);
                
                // Start new word if gap is too large
                if horizontal_gap > last.font_size * 0.3 {
                    if !current_word.is_empty() {
                        words.push(current_word.clone());
                        current_word.clear();
                    }
                }
            }
            
            current_word.push(char_data.clone());
            last_char = Some(char_data);
        }
        
        // Add the last word of the line
        if !current_word.is_empty() {
            words.push(current_word);
        }
    }
    
    words
}

/// Cluster words into lines based on baseline alignment
fn cluster_into_lines(word_clusters: &[Vec<CharacterData>]) -> Vec<Vec<Vec<CharacterData>>> {
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut last_baseline: Option<f32> = None;
    
    for word in word_clusters {
        if word.is_empty() {
            continue;
        }
        
        let word_baseline = word[0].baseline_y;
        
        if let Some(last_y) = last_baseline {
            // Check if this word is on the same line
            if (word_baseline - last_y).abs() > word[0].font_size * 0.5 {
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                }
            }
        }
        
        current_line.push(word.clone());
        last_baseline = Some(word_baseline);
    }
    
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    
    lines
}

/// Detect column boundaries based on consistent gaps across lines
fn detect_column_boundaries(lines: &[TextLine]) -> Vec<f32> {
    if lines.is_empty() {
        return Vec::new();
    }
    
    // Find vertical gaps that persist across multiple lines
    let mut gap_positions: HashMap<i32, usize> = HashMap::new();
    
    for line in lines {
        if line.chars.len() < 2 {
            continue;
        }
        
        // Look for gaps between characters
        for window in line.chars.windows(2) {
            let gap_start = window[0].x + window[0].width;
            let gap_end = window[1].x;
            let gap_size = gap_end - gap_start;
            
            // Significant gap that could be a column boundary
            if gap_size > window[0].font_size * COLUMN_GAP_THRESHOLD {
                // Bucket the position for fuzzy matching
                let bucket = (gap_start / 5.0) as i32;
                *gap_positions.entry(bucket).or_default() += 1;
            }
        }
    }
    
    // Return positions that appear in >60% of lines
    let min_frequency = (lines.len() as f32 * 0.6) as usize;
    let mut columns: Vec<f32> = gap_positions.into_iter()
        .filter(|(_, count)| *count >= min_frequency)
        .map(|(bucket, _)| bucket as f32 * 5.0)
        .collect();
    
    columns.sort_by_key(|x| OrderedFloat(*x));
    
    // Debug output
    if !columns.is_empty() {
        eprintln!("Detected {} columns at positions: {:?}", columns.len(), columns);
    }
    
    columns
}

/// Detect financial tables specifically
fn detect_financial_table(lines: &[TextLine], columns: &[f32]) -> Option<TableStructure> {
    // Look for lines with dollar signs or numeric patterns
    let dollar_lines: Vec<&TextLine> = lines.iter()
        .filter(|line| {
            let text: String = line.chars.iter().map(|c| c.unicode).collect();
            text.contains('$') || text.chars().filter(|c| c.is_ascii_digit()).count() >= 3
        })
        .collect();
    
    if dollar_lines.len() < MIN_TABLE_ROWS {
        return None;
    }
    
    // Look for year headers (2011-2015 pattern)
    let has_year_headers = lines.iter().any(|line| {
        let text: String = line.chars.iter().map(|c| c.unicode).collect();
        text.contains("2011") || text.contains("2012") || text.contains("2013")
    });
    
    if !has_year_headers && columns.len() < MIN_TABLE_COLS {
        return None;
    }
    
    eprintln!("Found financial table with {} rows and {} columns", dollar_lines.len(), columns.len());
    
    Some(TableStructure {
        columns: columns.to_vec(),
        rows: dollar_lines.iter().map(|l| l.baseline).collect(),
        cells: Vec::new(), // Will be populated by grid mapper
    })
}

/// Detect tables by finding aligned columns and rows (simplified for safety)
fn detect_tables_by_alignment(characters: &[CharacterData]) -> Vec<TableStructure> {
    // P1 SAFETY: Simplified table detection without complex clustering
    // This is a placeholder that avoids the expensive operations
    // that were causing the hang in EDIT tab
    
    // Quick check: if too many characters, skip table detection
    if characters.len() > 10_000 {
        return Vec::new(); // Too complex, skip table detection
    }
    
    let mut tables = Vec::new();
    
    // Simple heuristic: look for regular spacing patterns
    // Group characters by approximate X position (columns)
    let mut x_positions: Vec<f32> = Vec::new();
    for char in characters {
        // Round to nearest 5 pixels for grouping
        let rounded_x = (char.x / 5.0).round() * 5.0;
        if !x_positions.iter().any(|&x| (x - rounded_x).abs() < 2.0) {
            x_positions.push(rounded_x);
        }
    }
    x_positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    // Group characters by approximate Y position (rows)  
    let mut y_positions: Vec<f32> = Vec::new();
    for char in characters {
        // Round to nearest 3 pixels for grouping
        let rounded_y = (char.baseline_y / 3.0).round() * 3.0;
        if !y_positions.iter().any(|&y| (y - rounded_y).abs() < 2.0) {
            y_positions.push(rounded_y);
        }
    }
    y_positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    // Only consider it a table if we have a reasonable grid
    if x_positions.len() >= 3 && x_positions.len() <= 20 &&
       y_positions.len() >= 3 && y_positions.len() <= 100 {
        // Create a simple table structure
        tables.push(TableStructure {
            columns: x_positions,
            rows: y_positions,
            cells: vec![vec![String::new(); 0]; 0], // Empty cells for now
        });
    }
    
    tables
}

/// Map lines to grid without column awareness (for simple text)
fn map_lines_to_grid(
    grid: &mut Vec<Vec<char>>,
    lines: &[TextLine],
    width: usize,
    height: usize,
) {
    for (y, line) in lines.iter().enumerate() {
        if y >= height {
            break;
        }
        
        let mut x = 0;
        for ch in &line.chars {
            if x < width {
                grid[y][x] = ch.unicode;
                x += 1;
            }
        }
    }
}

/// Map lines to grid with an offset (for when we have images above)
fn map_lines_to_grid_with_offset(
    grid: &mut Vec<Vec<char>>,
    lines: &[TextLine],
    width: usize,
    height: usize,
    offset_y: usize,
) {
    for (y, line) in lines.iter().enumerate() {
        if offset_y + y >= height {
            break;
        }
        
        let mut x = 0;
        for ch in &line.chars {
            if x < width {
                grid[offset_y + y][x] = ch.unicode;
                x += 1;
            }
        }
    }
}

/// Map lines with natural spacing - preserve gaps but cap them at 1 line
fn map_lines_to_grid_with_natural_spacing(
    grid: &mut Vec<Vec<char>>,
    lines: &[TextLine],
    width: usize,
    height: usize,
    start_y: usize,
) {
    if lines.is_empty() {
        return;
    }
    
    let mut current_grid_y = start_y;
    let mut prev_baseline = lines[0].baseline;
    
    // Calculate average line spacing
    let mut spacings = Vec::new();
    for window in lines.windows(2) {
        let spacing = (window[1].baseline - window[0].baseline).abs();
        if spacing < 50.0 { // Normal spacing
            spacings.push(spacing);
        }
    }
    let avg_spacing = if !spacings.is_empty() {
        spacings.iter().sum::<f32>() / spacings.len() as f32
    } else {
        12.0
    };
    
    for line in lines {
        // Calculate how many lines to skip based on baseline difference
        let gap = ((line.baseline - prev_baseline) / avg_spacing) as usize;
        
        // Cap gaps at 1 line maximum
        let lines_to_skip = gap.min(1);
        current_grid_y += lines_to_skip;
        
        if current_grid_y >= height {
            break;
        }
        
        // Place characters
        let mut x = 0;
        for ch in &line.chars {
            if x < width {
                grid[current_grid_y][x] = ch.unicode;
                x += 1;
            }
        }
        
        current_grid_y += 1;
        prev_baseline = line.baseline;
    }
}

/// Map lines and tables to the character grid (legacy function)
fn map_to_grid_with_tables(
    grid: &mut Vec<Vec<char>>,
    lines: &[Vec<Vec<CharacterData>>],
    tables: &[TableStructure],
    grid_width: usize,
    grid_height: usize,
) {
    // First, render any detected tables with borders
    for table in tables {
        render_table_on_grid(grid, table, grid_width, grid_height);
    }
    
    // Then render regular text lines
    let mut current_y = 0;
    
    for line in lines {
        if current_y >= grid_height {
            break;
        }
        
        let mut current_x = 0;
        
        for word in line {
            for char_data in word {
                if current_x < grid_width && current_y < grid_height {
                    grid[current_y][current_x] = char_data.unicode;
                    current_x += 1;
                }
            }
            
            // Add space between words
            if current_x < grid_width {
                grid[current_y][current_x] = ' ';
                current_x += 1;
            }
        }
        
        current_y += 1;
    }
}

/// Render a table with ASCII borders on the grid
fn render_table_on_grid(
    grid: &mut Vec<Vec<char>>,
    table: &TableStructure,
    grid_width: usize,
    grid_height: usize,
) {
    // For now, just mark table regions
    // TODO: Implement proper table rendering with borders
    if !table.rows.is_empty() && !table.columns.is_empty() {
        let start_y = (table.rows[0] as usize).min(grid_height - 1);
        let end_y = (table.rows[table.rows.len() - 1] as usize).min(grid_height - 1);
        
        // Draw simple table markers
        for y in start_y..=end_y {
            if y < grid_height {
                grid[y][0] = '│';
                if grid_width > 1 {
                    grid[y][grid_width - 1] = '│';
                }
            }
        }
    }
}

fn place_text_on_grid_spatial(
    grid: &mut Vec<Vec<char>>, 
    text: &str, 
    x_start: usize, 
    y_start: usize, 
    max_width: usize,
    max_height: usize
) {
    let mut x = x_start;
    let mut y = y_start;
    
    for ch in text.chars() {
        // Handle newlines explicitly - preserve line breaks from source
        if ch == '\n' {
            y += 1;
            x = 0;  // Start new lines at the left edge
            if y >= max_height {
                break;
            }
            continue;
        }
        
        // Check if we need to wrap to next line
        if x >= max_width {
            // Simple wrapping - just move to next line
            y += 1;
            if y >= max_height {
                break;
            }
            x = 0;  // Start at left edge
        }
        
        // Place character on grid if within bounds
        if x < max_width && y < max_height {
            grid[y][x] = ch;
            x += 1;
        }
    }
}


pub async fn get_markdown_content(pdf_path: &Path, page_num: usize) -> Result<String> {
    // Use PDFium-based extraction with rich metadata
    let pdfium = crate::pdf_renderer::get_pdfium_instance();
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;
    
    // Extract characters with full metadata
    let characters = extract_characters_from_page(&page)?;
    
    // Extract annotations and form fields
    let annotations = extract_annotations(&page);
    let _form_fields = extract_form_fields(&page);
    
    // Build text lines with font information
    let text_lines = build_text_lines(&characters);
    
    // Convert to markdown with rich formatting
    let mut markdown = String::new();
    
    // Add page metadata if interesting
    let page_width = page.width().value;
    let page_height = page.height().value;
    
    // Add annotations section if present
    if !annotations.is_empty() {
        markdown.push_str("## 📝 Annotations\n\n");
        for annot in &annotations {
            if let Some(ref contents) = annot.contents {
                markdown.push_str(&format!("- **{}**: {}\n", annot.annotation_type, contents));
            } else {
                markdown.push_str(&format!("- **{}**\n", annot.annotation_type));
            }
        }
        markdown.push_str("\n---\n\n");
    }
    
    // Process text with font-aware formatting
    if !text_lines.is_empty() {
        let mut last_font_size = 0.0;
        let mut in_list = false;
        
        // Calculate average font size for the page
        let avg_font_size: f32 = text_lines.iter()
            .flat_map(|line| line.chars.iter().map(|c| c.font_size))
            .sum::<f32>() / text_lines.iter().map(|l| l.chars.len()).sum::<usize>() as f32;
        
        for (i, line) in text_lines.iter().enumerate() {
            if line.chars.is_empty() {
                continue;
            }
            
            // Get line text
            let line_text: String = line.chars.iter().map(|c| c.unicode).collect::<String>().trim().to_string();
            if line_text.is_empty() {
                continue;
            }
            
            // Get predominant font size for this line
            let line_font_size = if !line.chars.is_empty() {
                line.chars.iter().map(|c| c.font_size).sum::<f32>() / line.chars.len() as f32
            } else {
                avg_font_size
            };
            
            // Check if this line is bold (higher weight)
            let is_bold = line.chars.iter().any(|c| c.font_weight > 400);
            
            // Check if this line is italic
            let is_italic = line.chars.iter().any(|c| c.is_italic);
            
            // Detect headers based on font size
            if line_font_size > avg_font_size * 1.5 {
                // Large text - likely a title
                markdown.push_str(&format!("# {}\n\n", line_text));
                in_list = false;
            } else if line_font_size > avg_font_size * 1.2 || (is_bold && i == 0) {
                // Medium large text or bold at start - likely a section header
                markdown.push_str(&format!("## {}\n\n", line_text));
                in_list = false;
            } else if line_text.starts_with("•") || line_text.starts_with("-") || line_text.starts_with("*") || line_text.starts_with("·") {
                // List item
                markdown.push_str(&format!("{}\n", line_text));
                in_list = true;
            } else if line_text.chars().take(1).any(|c| c.is_ascii_digit()) && line_text.chars().nth(1) == Some('.') {
                // Numbered list
                markdown.push_str(&format!("{}\n", line_text));
                in_list = true;
            } else {
                // Regular text
                let mut formatted_text = line_text.clone();
                
                // Apply inline formatting
                if is_bold && is_italic {
                    formatted_text = format!("***{}***", formatted_text);
                } else if is_bold {
                    formatted_text = format!("**{}**", formatted_text);
                } else if is_italic {
                    formatted_text = format!("*{}*", formatted_text);
                }
                
                // Add as paragraph unless we're in a list
                if in_list {
                    markdown.push_str(&format!("  {}\n", formatted_text)); // Indent under list
                } else {
                    markdown.push_str(&format!("{}\n\n", formatted_text));
                }
                
                // Check for significant font size change
                if (line_font_size - last_font_size).abs() > avg_font_size * 0.3 {
                    in_list = false;
                }
            }
            
            last_font_size = line_font_size;
        }
    }
    
    // Add footer with extraction metadata
    markdown.push_str(&format!("\n---\n_Page {:.0}x{:.0} pts", page_width, page_height));
    if !annotations.is_empty() {
        markdown.push_str(&format!(" • {} annotations", annotations.len()));
    }
    markdown.push_str("_\n");
    
    if markdown.trim().is_empty() {
        markdown = "# 📄 No Content Found\n\n> No text content could be extracted from this page.\n\n**Try:**\n• Checking if the PDF contains text (not just images)\n• Using a different page\n• Enabling OCR if the PDF is scanned".to_string();
    }
    
    Ok(markdown)
}