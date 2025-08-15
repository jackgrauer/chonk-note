use anyhow::Result;
use pdfium_render::prelude::*;
use std::collections::{HashMap, BTreeMap};
use std::path::Path;

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

/// Represents a detected table structure
#[derive(Debug)]
struct TableStructure {
    rows: Vec<f32>,     // Y coordinates of row boundaries
    columns: Vec<f32>,  // X coordinates of column boundaries
    cells: Vec<Vec<String>>, // Cell contents
}

pub fn get_page_count(pdf_path: &Path) -> Result<usize> {
    // Use our pdf_renderer module which already has PDFium integration
    crate::pdf_renderer::get_pdf_page_count(pdf_path)
}

pub async fn extract_to_matrix(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Vec<char>>> {
    // Initialize grid
    let mut grid = vec![vec![' '; width]; height];
    
    // Use PDFium for character-level extraction
    let pdfium = crate::pdf_renderer::get_pdfium_instance();
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;
    
    // Extract characters with spatial data
    let characters = extract_characters_from_page(&page)?;
    
    // Detect tables using coordinate alignment
    let tables = detect_tables_by_alignment(&characters);
    
    // Cluster characters into words and lines
    let word_clusters = cluster_into_words(&characters);
    let line_clusters = cluster_into_lines(&word_clusters);
    
    // Map everything to the grid with proper spacing
    map_to_grid_with_tables(&mut grid, &line_clusters, &tables, width, height);
    
    Ok(grid)
}

/// Extract individual characters from a PDF page with spatial data
fn extract_characters_from_page(page: &PdfPage) -> Result<Vec<CharacterData>> {
    let mut characters = Vec::new();
    let text_page = page.text()?;
    
    // Get page dimensions for coordinate normalization
    let page_height = page.height().value;
    
    // Extract characters using the chars() method which returns a collection
    for char in text_page.chars().iter() {
        let unicode = match char.unicode_char() {
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

/// Cluster characters into words based on spatial proximity
fn cluster_into_words(characters: &[CharacterData]) -> Vec<Vec<CharacterData>> {
    let mut words = Vec::new();
    let mut current_word = Vec::new();
    let mut last_char: Option<&CharacterData> = None;
    
    // Sort characters by Y position first, then X position for reading order
    let mut sorted_chars = characters.to_vec();
    sorted_chars.sort_by(|a, b| {
        a.y.partial_cmp(&b.y).unwrap()
            .then(a.x.partial_cmp(&b.x).unwrap())
    });
    
    for char_data in &sorted_chars {
        if let Some(last) = last_char {
            // Check if this character is part of the same word
            let horizontal_gap = char_data.x - (last.x + last.width);
            let vertical_diff = (char_data.baseline_y - last.baseline_y).abs();
            
            // Start new word if gap is too large or on different line
            if horizontal_gap > last.font_size * 0.3 || vertical_diff > last.font_size * 0.5 {
                if !current_word.is_empty() {
                    words.push(current_word.clone());
                    current_word.clear();
                }
            }
        }
        
        current_word.push(char_data.clone());
        last_char = Some(char_data);
    }
    
    if !current_word.is_empty() {
        words.push(current_word);
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

/// Detect tables by finding aligned columns and rows
fn detect_tables_by_alignment(characters: &[CharacterData]) -> Vec<TableStructure> {
    let mut tables = Vec::new();
    
    // Group characters by similar X coordinates (potential columns)
    let mut x_groups: HashMap<i32, Vec<&CharacterData>> = HashMap::new();
    let x_tolerance = 3.0; // Tolerance for column alignment
    
    for char_data in characters {
        let bucket = (char_data.x / x_tolerance) as i32;
        x_groups.entry(bucket).or_default().push(char_data);
    }
    
    // Group characters by similar Y coordinates (potential rows)
    let mut y_groups: HashMap<i32, Vec<&CharacterData>> = HashMap::new();
    let y_tolerance = 2.0; // Tolerance for row alignment
    
    for char_data in characters {
        let bucket = (char_data.baseline_y / y_tolerance) as i32;
        y_groups.entry(bucket).or_default().push(char_data);
    }
    
    // Find potential table regions (areas with multiple aligned columns and rows)
    let column_positions: Vec<f32> = x_groups.iter()
        .filter(|(_, chars)| chars.len() > 3) // At least 3 characters in column
        .map(|(bucket, _)| *bucket as f32 * x_tolerance)
        .collect();
    
    let row_positions: Vec<f32> = y_groups.iter()
        .filter(|(_, chars)| chars.len() > 2) // At least 2 characters in row
        .map(|(bucket, _)| *bucket as f32 * y_tolerance)
        .collect();
    
    // If we have at least 3 columns and 3 rows, consider it a potential table
    if column_positions.len() >= 3 && row_positions.len() >= 3 {
        // For now, create a simple table structure
        // TODO: Implement more sophisticated table detection
        let num_cols = column_positions.len();
        let num_rows = row_positions.len();
        tables.push(TableStructure {
            columns: column_positions,
            rows: row_positions,
            cells: vec![vec![String::new(); num_cols]; num_rows],
        });
    }
    
    tables
}

/// Map lines and tables to the character grid
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
    // For now, keep using Ferrules for markdown generation
    // TODO: Convert to PDFium-based extraction
    use ferrules_core::{
        blocks::BlockType,
        FerrulesParseConfig, FerrulesParser,
        layout::model::ORTConfig,
    };
    
    let ort_config = ORTConfig::default();
    let parser = FerrulesParser::new(ort_config);
    
    let parse_config = FerrulesParseConfig {
        password: None,
        flatten_pdf: true,
        page_range: Some(page_num..page_num + 1),
        debug_dir: None,
    };
    
    let pdf_bytes = tokio::fs::read(pdf_path).await?;
    let parsed_doc = parser.parse_document(
        &pdf_bytes,
        pdf_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string(),
        parse_config,
        None::<fn(usize)>,
    ).await?;
    
    // Table extraction disabled - was causing crashes
    
    // Convert parsed document blocks to markdown with better spatial awareness
    let mut markdown = String::new();
    
    if let Some(page) = parsed_doc.pages.first() {
        let page_id = page.id;
        let page_height = page.height;
        
        // Collect blocks with their positions
        let mut positioned_blocks: Vec<(&ferrules_core::blocks::Block, f32)> = Vec::new();
        
        for block in &parsed_doc.blocks {
            if block.pages_id.contains(&page_id) {
                // Use y position for sorting (top to bottom)
                positioned_blocks.push((block, block.bbox.y0));
            }
        }
        
        // Sort by vertical position to preserve reading order
        positioned_blocks.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        
        let mut last_y = 0.0;
        
        for (block, y_pos) in positioned_blocks {
            // Add extra spacing if there's a significant vertical gap
            if last_y > 0.0 && (y_pos - last_y) > page_height * 0.05 {
                markdown.push_str("\n");
            }
            last_y = y_pos + (block.bbox.y1 - block.bbox.y0);
            
            match &block.kind {
                BlockType::Title(t) => {
                    // Title without excessive formatting
                    markdown.push_str(&format!("# {}\n\n", t.text.trim()));
                }
                BlockType::Header(h) => {
                    // Section header with better formatting
                    markdown.push_str(&format!("## {}\n\n", h.text.trim()));
                }
                BlockType::Table => {
                    // Ferrules says there's a table but doesn't extract it
                    markdown.push_str("[Table content not extracted]\n\n");
                }
                BlockType::TextBlock(tb) => {
                    let text = tb.text.trim();
                    
                    // Just handle special markdown formatting, no table detection
                    if text.lines().any(|line| line.starts_with("    ") || line.starts_with("\t")) {
                        // Code block
                        markdown.push_str("```\n");
                        markdown.push_str(text);
                        markdown.push_str("\n```\n\n");
                    } else if text.starts_with("Note:") || text.starts_with("NOTE:") {
                        // Note blocks
                        markdown.push_str(&format!("> **Note:** {}\n\n", &text[5..].trim()));
                    } else if text.starts_with("Warning:") || text.starts_with("WARNING:") {
                        // Warning blocks
                        markdown.push_str(&format!("> ⚠️ **Warning:** {}\n\n", &text[8..].trim()));
                    } else {
                        // Regular paragraph
                        markdown.push_str(&format!("{}\n\n", text));
                    }
                }
                BlockType::ListBlock(l) => {
                    // Enhanced list formatting
                    markdown.push_str("\n");
                    for (i, item) in l.items.iter().enumerate() {
                        // Use different bullet styles for variety
                        let bullet = if i % 3 == 0 { "•" } else if i % 3 == 1 { "▸" } else { "◦" };
                        markdown.push_str(&format!("{} {}\n", bullet, item.trim()));
                    }
                    markdown.push_str("\n");
                }
                BlockType::Footer(f) => {
                    // Footer with better formatting
                    markdown.push_str("\n---\n\n");
                    markdown.push_str(&format!("_{}_\n\n", f.text.trim()));
                }
                _ => {}
            }
        }
    }
    
    if markdown.is_empty() {
        markdown = "# 📄 No Content Found\n\n> No text content could be extracted from this page.\n\n**Try:**\n• Checking if the PDF contains text (not just images)\n• Using a different page\n• Enabling OCR if the PDF is scanned".to_string();
    }
    
    Ok(markdown)
}