use anyhow::Result;
use pdfium_render::prelude::*;
use std::path::Path;
use std::collections::BTreeMap;

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

/// Extract tables using PDFium's text segment API (much faster)
pub fn extract_tables_from_page_fast(pdf_path: &Path, page_num: usize) -> Result<Vec<Table>> {
    #[cfg(debug_assertions)]
    eprintln!("Starting fast table extraction for page {}", page_num);
    
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
    
    // Get text page
    let text_page = page.text()?;
    
    // Use segments instead of individual characters - MUCH faster!
    let segments = text_page.segments();
    
    #[cfg(debug_assertions)]
    eprintln!("Processing {} text segments", segments.len());
    
    // Collect text segments with their positions
    let mut text_segments = Vec::new();
    
    for segment in segments.iter() {
        // Get the text content
        let text = segment.text();
        
        // Skip empty segments
        if text.trim().is_empty() {
            continue;
        }
        
        // Get the bounding rectangle for this segment
        let bounds = segment.bounds();
        text_segments.push((
            text.clone(),
            bounds.left.value,
            bounds.top.value,
            bounds.right.value - bounds.left.value,
            bounds.bottom.value - bounds.top.value,
        ));
    }
    
    #[cfg(debug_assertions)]
    eprintln!("Collected {} non-empty text segments", text_segments.len());
    
    // Now detect tables from segments
    let tables = detect_tables_from_segments_fast(&text_segments);
    
    #[cfg(debug_assertions)]
    eprintln!("Found {} potential tables", tables.len());
    
    Ok(tables)
}

/// Fast table detection using segment grouping
fn detect_tables_from_segments_fast(segments: &[(String, f32, f32, f32, f32)]) -> Vec<Table> {
    let mut tables = Vec::new();
    
    // Early exit if too few segments
    if segments.len() < 4 {
        return tables;
    }
    
    // Group segments by Y position (rows) using a tolerance
    let row_tolerance = 5.0;
    let mut rows: BTreeMap<i32, Vec<(String, f32, f32, f32, f32)>> = BTreeMap::new();
    
    for segment in segments {
        // Use integer key for efficient grouping
        let y_key = (segment.2 / row_tolerance) as i32;
        rows.entry(y_key).or_insert_with(Vec::new).push(segment.clone());
    }
    
    // Sort segments within each row by X position
    for row in rows.values_mut() {
        row.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    }
    
    // Convert to vec for easier processing
    let mut sorted_rows: Vec<Vec<(String, f32, f32, f32, f32)>> = rows.into_values().collect();
    
    // Look for table patterns (consecutive rows with similar column alignment)
    let mut current_table_rows = Vec::new();
    let mut prev_col_count = 0;
    
    for row in sorted_rows {
        let col_count = row.len();
        
        // Check if this could be part of a table
        if col_count >= 2 {
            // Check if column count is similar to previous row
            if prev_col_count == 0 || (col_count as i32 - prev_col_count as i32).abs() <= 1 {
                current_table_rows.push(row);
                prev_col_count = col_count;
                
                // Create table if we have enough rows
                if current_table_rows.len() >= 3 {
                    // Check if this looks like a table by examining alignment
                    if is_likely_table(&current_table_rows) {
                        if let Some(table) = create_table_from_rows_fast(&current_table_rows) {
                            tables.push(table);
                            current_table_rows.clear();
                            prev_col_count = 0;
                        }
                    }
                }
            } else {
                // Column count changed significantly, end current table
                if current_table_rows.len() >= 2 && is_likely_table(&current_table_rows) {
                    if let Some(table) = create_table_from_rows_fast(&current_table_rows) {
                        tables.push(table);
                    }
                }
                current_table_rows.clear();
                current_table_rows.push(row);
                prev_col_count = col_count;
            }
        } else {
            // Single column, not a table row
            if current_table_rows.len() >= 2 && is_likely_table(&current_table_rows) {
                if let Some(table) = create_table_from_rows_fast(&current_table_rows) {
                    tables.push(table);
                }
            }
            current_table_rows.clear();
            prev_col_count = 0;
        }
    }
    
    // Handle last potential table
    if current_table_rows.len() >= 2 && is_likely_table(&current_table_rows) {
        if let Some(table) = create_table_from_rows_fast(&current_table_rows) {
            tables.push(table);
        }
    }
    
    tables
}

/// Quick check if rows look like a table
fn is_likely_table(rows: &[Vec<(String, f32, f32, f32, f32)>]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    
    // Check for consistent column alignment
    let first_row = &rows[0];
    let tolerance = 15.0;
    
    for row in rows.iter().skip(1) {
        // Check if columns roughly align
        if row.len() != first_row.len() {
            continue;
        }
        
        let mut aligned = true;
        for (col1, col2) in first_row.iter().zip(row.iter()) {
            if (col1.1 - col2.1).abs() > tolerance {
                aligned = false;
                break;
            }
        }
        
        if aligned {
            return true;
        }
    }
    
    false
}

/// Create a Table struct from aligned rows (fast version)
fn create_table_from_rows_fast(rows: &[Vec<(String, f32, f32, f32, f32)>]) -> Option<Table> {
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
            markdown.push_str(&format!(" {} |", cell.text.trim()));
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
            markdown.push_str(&format!(" {} |", cell.text.trim()));
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
                line.push_str(&format!(" {:<width$} |", text.trim(), width = w - 2));
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