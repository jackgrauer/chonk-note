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

/// Character data extracted from PDFium with full spatial information
#[derive(Debug, Clone)]
struct CharacterData {
    unicode: char,
    x: f32,           // Position in PDF coordinates
    y: f32,
    width: f32,       // Character width
    height: f32,      // Character height
    font_size: f32,
    baseline_y: f32,  // Text baseline for alignment
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
    
    // Extract characters with spatial data
    let characters = extract_characters_from_page(&page)?;
    
    // Safety check: Prevent processing too many characters
    if characters.len() > MAX_CHARS_PER_PAGE {
        // Fall back to simple extraction for complex pages
        return Ok(simple_text_fallback(&characters, width, height));
    }
    
    // Build text lines with proper baseline grouping
    let text_lines = build_text_lines(&characters);
    
    // Detect column boundaries for table preservation
    let columns = detect_column_boundaries(&text_lines);
    
    // Check if this is a financial table
    let _table_structure = detect_financial_table(&text_lines, &columns);
    
    // DECISION POINT: Column-aware vs Sequential
    // ============================================
    // This is where we choose between two extraction strategies:
    // - Column-aware: Better for tables but may mess up regular text
    // - Sequential: Better for text but tables become unreadable
    //
    // To switch to simple sequential (v7.27 style):
    // Comment out the column detection and always use map_lines_to_grid
    //
    // Current: Use column-aware when columns detected (favors tables)
    if !columns.is_empty() && columns.len() >= 2 {
        // We have columns - use column-aware mapper
        let mapper = ColumnAwareGridMapper::new(columns, width);
        grid = mapper.map_to_grid(&text_lines, width, height);
    } else {
        // No columns detected - use simple line-based mapping
        map_lines_to_grid(&mut grid, &text_lines, width, height);
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
        
        // Estimate font size from character height
        let font_size = bounds.height().value;
        
        characters.push(CharacterData {
            unicode,
            x: bounds.left().value,
            y: page_height - bounds.top().value, // Convert to top-down coordinates
            width: bounds.width().value,
            height: bounds.height().value,
            font_size,
            baseline_y: page_height - bounds.bottom().value,
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
    // Use PDFium-based extraction instead of ferrules
    let pdfium = crate::pdf_renderer::get_pdfium_instance();
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;
    
    // Extract text using PDFium
    let text_page = page.text()?;
    let text = text_page.all();
    
    // Convert to markdown with simple formatting
    let mut markdown = String::new();
    
    if !text.trim().is_empty() {
        // Split into paragraphs and format
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        
        for (i, para) in paragraphs.iter().enumerate() {
            let trimmed = para.trim();
            if trimmed.is_empty() {
                continue;
            }
            
            // Simple heuristics for formatting
            if i == 0 && trimmed.len() < 100 && !trimmed.contains('.') {
                // Likely a title
                markdown.push_str(&format!("# {}\n\n", trimmed));
            } else if trimmed.len() < 80 && trimmed.chars().filter(|c| c.is_uppercase()).count() > trimmed.len() / 3 {
                // Likely a header (lots of caps)
                markdown.push_str(&format!("## {}\n\n", trimmed));
            } else if trimmed.starts_with("•") || trimmed.starts_with("-") || trimmed.starts_with("*") {
                // List item
                markdown.push_str(&format!("{}\n", trimmed));
            } else {
                // Regular paragraph
                markdown.push_str(&format!("{}\n\n", trimmed));
            }
        }
    }
    
    if markdown.is_empty() {
        markdown = "# 📄 No Content Found\n\n> No text content could be extracted from this page.\n\n**Try:**\n• Checking if the PDF contains text (not just images)\n• Using a different page\n• Enabling OCR if the PDF is scanned".to_string();
    }
    
    Ok(markdown)
}