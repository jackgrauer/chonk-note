use anyhow::Result;
use pdfium_render::prelude::*;
use std::path::Path;
use std::collections::{BTreeMap, HashMap};

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

#[derive(Debug, Clone)]
struct TextSegment {
    text: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    right: f32,
    bottom: f32,
}

/// Advanced table extraction using improved algorithms
pub fn extract_tables_advanced(pdf_path: &Path, page_num: usize) -> Result<Vec<Table>> {
    #[cfg(debug_assertions)]
    eprintln!("Starting advanced table extraction for page {}", page_num);
    
    // Create PDFium instance
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path("./lib/")
        )?
    );
    
    // Load the PDF document
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let pages = document.pages();
    let page = pages.get(page_num as u16)?;
    
    // Get text page
    let text_page = page.text()?;
    
    // Collect all text segments
    let segments = collect_text_segments(&text_page);
    
    #[cfg(debug_assertions)]
    eprintln!("Collected {} text segments", segments.len());
    
    // Find tables using advanced detection
    let tables = detect_tables_advanced(&segments);
    
    #[cfg(debug_assertions)]
    eprintln!("Found {} tables", tables.len());
    
    Ok(tables)
}

/// Collect text segments from the page
fn collect_text_segments(text_page: &PdfPageText) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    
    // Use rectangles API for better text grouping
    let rect_count = text_page.rect_count();
    
    #[cfg(debug_assertions)]
    eprintln!("Processing {} text rectangles", rect_count);
    
    for i in 0..rect_count {
        if let Ok(rect) = text_page.get_rect(i) {
            // Get text within this rectangle
            if let Ok(text) = text_page.get_text_from_rect(&rect) {
                let text = text.trim();
                if !text.is_empty() {
                    segments.push(TextSegment {
                        text: text.to_string(),
                        x: rect.left.value,
                        y: rect.top.value,
                        width: rect.right.value - rect.left.value,
                        height: rect.bottom.value - rect.top.value,
                        right: rect.right.value,
                        bottom: rect.bottom.value,
                    });
                }
            }
        }
    }
    
    // If rectangles didn't work well, fall back to segments
    if segments.is_empty() {
        #[cfg(debug_assertions)]
        eprintln!("No rectangles found, falling back to segments");
        
        for segment in text_page.segments().iter() {
            let text = segment.text();
            if !text.trim().is_empty() {
                let bounds = segment.bounds();
                segments.push(TextSegment {
                    text: text.clone(),
                    x: bounds.left.value,
                    y: bounds.top.value,
                    width: bounds.right.value - bounds.left.value,
                    height: bounds.bottom.value - bounds.top.value,
                    right: bounds.right.value,
                    bottom: bounds.bottom.value,
                });
            }
        }
    }
    
    segments
}

/// Advanced table detection using clustering and alignment
fn detect_tables_advanced(segments: &[TextSegment]) -> Vec<Table> {
    let mut tables = Vec::new();
    
    if segments.len() < 4 {
        return tables;
    }
    
    // Step 1: Cluster segments into rows based on Y position
    let rows = cluster_into_rows(segments);
    
    // Step 2: Find potential table regions
    let table_regions = find_table_regions(&rows);
    
    // Step 3: Build tables from regions
    for region in table_regions {
        if let Some(table) = build_table_from_region(&region) {
            tables.push(table);
        }
    }
    
    tables
}

/// Cluster text segments into rows based on Y position
fn cluster_into_rows(segments: &[TextSegment]) -> Vec<Vec<TextSegment>> {
    // Group by similar Y position with adaptive threshold
    let mut row_map: BTreeMap<i32, Vec<TextSegment>> = BTreeMap::new();
    
    for segment in segments {
        // Use adaptive clustering based on text height
        let row_key = (segment.y / (segment.height * 0.5).max(5.0)) as i32;
        row_map.entry(row_key).or_insert_with(Vec::new).push(segment.clone());
    }
    
    // Convert to vector and sort segments within each row by X
    let mut rows: Vec<Vec<TextSegment>> = row_map.into_values().collect();
    for row in &mut rows {
        row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    }
    
    rows
}

/// Find regions that look like tables
fn find_table_regions(rows: &[Vec<TextSegment>]) -> Vec<Vec<Vec<TextSegment>>> {
    let mut regions = Vec::new();
    let mut current_region = Vec::new();
    let mut prev_columns = Vec::new();
    
    for row in rows {
        if row.len() < 2 {
            // Single column - not part of a table
            if current_region.len() >= 2 {
                regions.push(current_region.clone());
            }
            current_region.clear();
            prev_columns.clear();
            continue;
        }
        
        // Extract column positions
        let columns: Vec<f32> = row.iter().map(|s| s.x).collect();
        
        if prev_columns.is_empty() {
            // Start of potential table
            current_region.push(row.clone());
            prev_columns = columns;
        } else {
            // Check if columns align with previous row
            if columns_align(&columns, &prev_columns) {
                current_region.push(row.clone());
                prev_columns = merge_column_positions(&prev_columns, &columns);
            } else {
                // Column mismatch - end current table
                if current_region.len() >= 2 {
                    regions.push(current_region.clone());
                }
                current_region.clear();
                current_region.push(row.clone());
                prev_columns = columns;
            }
        }
    }
    
    // Don't forget the last region
    if current_region.len() >= 2 {
        regions.push(current_region);
    }
    
    regions
}

/// Check if two sets of columns align
fn columns_align(cols1: &[f32], cols2: &[f32]) -> bool {
    // Allow different column counts (merged cells)
    let min_cols = cols1.len().min(cols2.len());
    
    if min_cols < 2 {
        return false;
    }
    
    // Check if at least 60% of columns align
    let mut aligned = 0;
    let tolerance = 15.0;
    
    for i in 0..min_cols {
        if (cols1[i] - cols2[i]).abs() < tolerance {
            aligned += 1;
        }
    }
    
    aligned as f32 / min_cols as f32 > 0.6
}

/// Merge column positions for better alignment
fn merge_column_positions(cols1: &[f32], cols2: &[f32]) -> Vec<f32> {
    let mut merged = Vec::new();
    let tolerance = 15.0;
    
    // Start with all positions from cols1
    merged.extend_from_slice(cols1);
    
    // Add any new positions from cols2
    for &col2 in cols2 {
        let mut found = false;
        for &col1 in cols1 {
            if (col1 - col2).abs() < tolerance {
                found = true;
                break;
            }
        }
        if !found {
            merged.push(col2);
        }
    }
    
    merged.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    merged
}

/// Build a table from a region of aligned rows
fn build_table_from_region(region: &[Vec<TextSegment>]) -> Option<Table> {
    if region.is_empty() {
        return None;
    }
    
    // Determine column boundaries
    let column_boundaries = determine_column_boundaries(region);
    
    let mut cells = Vec::new();
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    
    for (row_idx, row) in region.iter().enumerate() {
        let mut row_cells = Vec::new();
        
        // Assign segments to columns
        for segment in row {
            let col_idx = find_column_index(&segment, &column_boundaries);
            
            let cell = TableCell {
                text: segment.text.clone(),
                row: row_idx,
                col: col_idx,
                x: segment.x,
                y: segment.y,
                width: segment.width,
                height: segment.height,
            };
            
            min_x = min_x.min(segment.x);
            min_y = min_y.min(segment.y);
            max_x = max_x.max(segment.right);
            max_y = max_y.max(segment.bottom);
            
            row_cells.push(cell);
        }
        
        // Sort cells by column index
        row_cells.sort_by_key(|c| c.col);
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

/// Determine column boundaries from all segments in a region
fn determine_column_boundaries(region: &[Vec<TextSegment>]) -> Vec<f32> {
    let mut all_x_positions = Vec::new();
    
    for row in region {
        for segment in row {
            all_x_positions.push(segment.x);
            all_x_positions.push(segment.right);
        }
    }
    
    all_x_positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    // Cluster X positions to find column boundaries
    let mut boundaries = Vec::new();
    let mut last_x = -1000.0;
    let min_col_width = 20.0;
    
    for &x in &all_x_positions {
        if x - last_x > min_col_width {
            boundaries.push(x);
            last_x = x;
        }
    }
    
    boundaries
}

/// Find which column a segment belongs to
fn find_column_index(segment: &TextSegment, boundaries: &[f32]) -> usize {
    let center_x = segment.x + segment.width / 2.0;
    
    for (i, &boundary) in boundaries.iter().enumerate() {
        if center_x < boundary {
            return i.saturating_sub(1);
        }
    }
    
    boundaries.len().saturating_sub(1)
}

/// Format a table as markdown
pub fn table_to_markdown(table: &Table) -> String {
    if table.cells.is_empty() {
        return String::new();
    }
    
    let mut markdown = String::new();
    
    // Find maximum columns
    let max_cols = table.cells.iter()
        .flat_map(|row| row.iter())
        .map(|cell| cell.col + 1)
        .max()
        .unwrap_or(0);
    
    // Build rows with proper column alignment
    for (row_idx, row) in table.cells.iter().enumerate() {
        // Create cells array for this row
        let mut cells_text = vec![String::new(); max_cols];
        
        for cell in row {
            if cell.col < max_cols {
                cells_text[cell.col] = cell.text.trim().to_string();
            }
        }
        
        // Output row
        markdown.push('|');
        for text in &cells_text {
            markdown.push_str(&format!(" {} |", text));
        }
        markdown.push('\n');
        
        // Add separator after first row
        if row_idx == 0 {
            markdown.push('|');
            for _ in 0..max_cols {
                markdown.push_str(" --- |");
            }
            markdown.push('\n');
        }
    }
    
    markdown
}

/// Format table for character grid display
pub fn table_to_grid(table: &Table, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    
    if table.cells.is_empty() {
        return lines;
    }
    
    // Find maximum columns
    let max_cols = table.cells.iter()
        .flat_map(|row| row.iter())
        .map(|cell| cell.col + 1)
        .max()
        .unwrap_or(0);
    
    // Calculate column widths
    let mut col_widths = vec![3; max_cols]; // Minimum width
    
    for row in &table.cells {
        for cell in row {
            if cell.col < max_cols {
                col_widths[cell.col] = col_widths[cell.col].max(cell.text.len() + 2);
            }
        }
    }
    
    // Ensure total width fits
    let total_width: usize = col_widths.iter().sum::<usize>() + max_cols + 1;
    if total_width > width {
        // Scale down proportionally
        let scale = width as f32 / total_width as f32;
        for w in &mut col_widths {
            *w = ((*w as f32) * scale) as usize;
            *w = (*w).max(3);
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
        let mut cells_text = vec![String::new(); max_cols];
        
        // Fill cells array
        for cell in row {
            if cell.col < max_cols {
                cells_text[cell.col] = cell.text.trim().to_string();
            }
        }
        
        // Output cells
        for (col_idx, text) in cells_text.iter().enumerate() {
            let w = col_widths[col_idx];
            let display_text = if text.len() > w - 2 {
                &text[..w - 2]
            } else {
                text
            };
            line.push_str(&format!(" {:<width$} |", display_text, width = w - 2));
        }
        
        lines.push(line);
    }
    
    // Draw bottom border
    lines.push(border);
    
    lines
}