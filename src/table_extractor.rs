use anyhow::Result;
use pdfium_render::prelude::*;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TableCell {
    pub text: String,
    pub row: usize,
    pub col: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug)]
pub struct Table {
    pub cells: Vec<Vec<TableCell>>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Extract tables from a PDF page using PDFium's text extraction
pub fn extract_tables_from_page(pdf_path: &Path, page_num: usize) -> Result<Vec<Table>> {
    // Create PDFium instance
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path("./lib/")
        )?
    );
    
    // Load the PDF document
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    
    // Get the requested page
    let pages = document.pages();
    let page = pages.get(page_num as u16)?;
    
    // Extract text with positional information
    let text_page = page.text()?;
    
    // Get all text segments with their positions
    let mut text_segments = Vec::new();
    
    // PDFium provides character-level access, so we need to group into words/segments
    let char_count = text_page.chars().len();
    let mut current_word = String::new();
    let mut word_start_x = 0.0;
    let mut word_start_y = 0.0;
    let mut word_end_x = 0.0;
    
    for i in 0..char_count {
        match text_page.chars().get(i) {
            Ok(char_obj) => {
                // Get the character (returns Option<char>)
                if let Some(ch) = char_obj.unicode_char() {
                    // Get character position using loose bounds
                    if let Ok(loose_bounds) = char_obj.loose_bounds() {
                        if ch.is_whitespace() || ch == '\n' || ch == '\r' {
                            // End of word - save it if we have content
                            if !current_word.is_empty() {
                                text_segments.push((
                                    current_word.clone(),
                                    word_start_x,
                                    word_start_y,
                                    word_end_x - word_start_x,
                                    loose_bounds.bottom.value - word_start_y,
                                ));
                                current_word.clear();
                            }
                        } else {
                            // Add to current word
                            if current_word.is_empty() {
                                word_start_x = loose_bounds.left.value;
                                word_start_y = loose_bounds.top.value;
                            }
                            current_word.push(ch);
                            word_end_x = loose_bounds.right.value;
                        }
                    }
                }
            }
            Err(_) => {
                // Skip characters we can't read
                continue;
            }
        }
    }
    
    // Save last word if any
    if !current_word.is_empty() {
        text_segments.push((
            current_word,
            word_start_x,
            word_start_y,
            word_end_x - word_start_x,
            0.0, // Height will be calculated
        ));
    }
    
    // Now detect tables by looking for aligned text segments
    let tables = detect_tables_from_segments(&text_segments);
    
    Ok(tables)
}

/// Detect tables from text segments by looking for alignment patterns
fn detect_tables_from_segments(segments: &[(String, f32, f32, f32, f32)]) -> Vec<Table> {
    let mut tables = Vec::new();
    
    // Group segments by approximate Y position (rows)
    let mut rows: Vec<Vec<(String, f32, f32, f32, f32)>> = Vec::new();
    let row_threshold = 5.0; // Points tolerance for same row
    
    for segment in segments {
        let y = segment.2;
        
        // Find matching row or create new one
        let mut found_row = false;
        for row in &mut rows {
            if !row.is_empty() {
                let row_y = row[0].2;
                if (y - row_y).abs() < row_threshold {
                    row.push(segment.clone());
                    found_row = true;
                    break;
                }
            }
        }
        
        if !found_row {
            rows.push(vec![segment.clone()]);
        }
    }
    
    // Sort rows by Y position
    rows.sort_by(|a, b| {
        let y_a = if !a.is_empty() { a[0].2 } else { 0.0 };
        let y_b = if !b.is_empty() { b[0].2 } else { 0.0 };
        y_a.partial_cmp(&y_b).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    // Sort segments within each row by X position
    for row in &mut rows {
        row.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    }
    
    // Detect tables by looking for consistent column alignment
    let mut potential_tables = Vec::new();
    let mut current_table_rows = Vec::new();
    let mut column_positions: Vec<f32> = Vec::new();
    
    for (_row_idx, row) in rows.iter().enumerate() {
        if row.len() < 2 {
            // Single item rows typically aren't part of tables
            if !current_table_rows.is_empty() && current_table_rows.len() >= 2 {
                // Save current table if it has at least 2 rows
                potential_tables.push(current_table_rows.clone());
            }
            current_table_rows.clear();
            column_positions.clear();
            continue;
        }
        
        // Extract X positions
        let x_positions: Vec<f32> = row.iter().map(|s| s.1).collect();
        
        if column_positions.is_empty() {
            // First row of potential table
            column_positions = x_positions.clone();
            current_table_rows.push(row.clone());
        } else {
            // Check if this row aligns with previous columns
            let aligns = check_column_alignment(&x_positions, &column_positions, 10.0);
            
            if aligns {
                current_table_rows.push(row.clone());
                // Update column positions with average
                update_column_positions(&mut column_positions, &x_positions);
            } else {
                // End of table
                if current_table_rows.len() >= 2 {
                    potential_tables.push(current_table_rows.clone());
                }
                current_table_rows.clear();
                column_positions = x_positions;
                current_table_rows.push(row.clone());
            }
        }
    }
    
    // Save last table if any
    if current_table_rows.len() >= 2 {
        potential_tables.push(current_table_rows);
    }
    
    // Convert potential tables to Table structs
    for table_rows in potential_tables {
        if let Some(table) = create_table_from_rows(&table_rows) {
            tables.push(table);
        }
    }
    
    tables
}

/// Check if two sets of column positions align
fn check_column_alignment(positions1: &[f32], positions2: &[f32], tolerance: f32) -> bool {
    if positions1.len() != positions2.len() {
        // Different number of columns might still be a table with merged cells
        // For now, require same number of columns
        return false;
    }
    
    for (p1, p2) in positions1.iter().zip(positions2.iter()) {
        if (p1 - p2).abs() > tolerance {
            return false;
        }
    }
    
    true
}

/// Update column positions with weighted average
fn update_column_positions(positions: &mut [f32], new_positions: &[f32]) {
    for (pos, new_pos) in positions.iter_mut().zip(new_positions.iter()) {
        *pos = (*pos + *new_pos) / 2.0;
    }
}

/// Create a Table struct from aligned rows
fn create_table_from_rows(rows: &[Vec<(String, f32, f32, f32, f32)>]) -> Option<Table> {
    if rows.is_empty() {
        return None;
    }
    
    let mut cells = Vec::new();
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    
    for (row_idx, row) in rows.iter().enumerate() {
        let mut row_cells = Vec::new();
        
        for (col_idx, segment) in row.iter().enumerate() {
            let cell = TableCell {
                text: segment.0.clone(),
                row: row_idx,
                col: col_idx,
                x: segment.1,
                y: segment.2,
                width: segment.3,
                height: segment.4,
            };
            
            min_x = min_x.min(segment.1);
            min_y = min_y.min(segment.2);
            max_x = max_x.max(segment.1 + segment.3);
            max_y = max_y.max(segment.2 + segment.4);
            
            row_cells.push(cell);
        }
        
        cells.push(row_cells);
    }
    
    Some(Table {
        cells,
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    })
}

/// Format a table as markdown
pub fn table_to_markdown(table: &Table) -> String {
    if table.cells.is_empty() {
        return String::new();
    }
    
    let mut markdown = String::new();
    
    // Find maximum columns
    let max_cols = table.cells.iter().map(|row| row.len()).max().unwrap_or(0);
    
    // First row as header
    if let Some(header_row) = table.cells.first() {
        markdown.push('|');
        for cell in header_row {
            markdown.push_str(&format!(" {} |", cell.text));
        }
        // Fill empty columns if needed
        for _ in header_row.len()..max_cols {
            markdown.push_str(" |");
        }
        markdown.push('\n');
        
        // Separator row
        markdown.push('|');
        for _ in 0..max_cols {
            markdown.push_str(" --- |");
        }
        markdown.push('\n');
    }
    
    // Data rows
    for row in table.cells.iter().skip(1) {
        markdown.push('|');
        for cell in row {
            markdown.push_str(&format!(" {} |", cell.text));
        }
        // Fill empty columns if needed
        for _ in row.len()..max_cols {
            markdown.push_str(" |");
        }
        markdown.push('\n');
    }
    
    markdown
}

/// Format table for character grid display
pub fn table_to_grid(table: &Table, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    
    if table.cells.is_empty() {
        return lines;
    }
    
    // Calculate column widths
    let mut col_widths = Vec::new();
    for row in &table.cells {
        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx >= col_widths.len() {
                col_widths.push(cell.text.len());
            } else {
                col_widths[col_idx] = col_widths[col_idx].max(cell.text.len());
            }
        }
    }
    
    // Add padding
    for width in &mut col_widths {
        *width += 2; // 1 space padding on each side
    }
    
    // Ensure total width fits
    let total_width: usize = col_widths.iter().sum::<usize>() + col_widths.len() + 1;
    if total_width > width {
        // Scale down column widths proportionally
        let scale = width as f32 / total_width as f32;
        for w in &mut col_widths {
            *w = ((*w as f32) * scale) as usize;
            *w = (*w).max(3); // Minimum 3 chars per column
        }
    }
    
    // Draw top border
    let mut border = String::from("+");
    for w in &col_widths {
        border.push_str(&"-".repeat(*w));
        border.push('+');
    }
    lines.push(border.clone());
    
    // Draw rows
    for row in &table.cells {
        let mut line = String::from("|");
        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx < col_widths.len() {
                let w = col_widths[col_idx];
                let text = if cell.text.len() > w - 2 {
                    &cell.text[..w - 2]
                } else {
                    &cell.text
                };
                line.push_str(&format!(" {:<width$} |", text, width = w - 2));
            }
        }
        // Fill empty columns
        for col_idx in row.len()..col_widths.len() {
            let w = col_widths[col_idx];
            line.push_str(&format!(" {:<width$} |", "", width = w - 2));
        }
        lines.push(line);
    }
    
    // Draw bottom border
    lines.push(border);
    
    lines
}