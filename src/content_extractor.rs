use anyhow::Result;
use ferrules_core::{
    blocks::BlockType,
    FerrulesParseConfig, FerrulesParser,
    layout::model::ORTConfig,
};
use std::path::Path;
use crate::table_extractor;
// SpatialTextGrid removed - implementation incomplete

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
    
    // Configure ferrules parser
    let ort_config = ORTConfig::default();
    let parser = FerrulesParser::new(ort_config);
    
    // Parse configuration - only parse the requested page
    let parse_config = FerrulesParseConfig {
        password: None,
        flatten_pdf: true,
        page_range: Some(page_num..page_num + 1),
        debug_dir: None,
    };
    
    // Read PDF file
    let pdf_bytes = tokio::fs::read(pdf_path).await?;
    
    // Parse the document
    let parsed_doc = parser.parse_document(
        &pdf_bytes,
        pdf_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string(),
        parse_config,
        None::<fn(usize)>,
    ).await?;
    
    // TEMPORARILY DISABLED: Table extraction is too slow and causes freezing
    // TODO: Optimize table extraction or run it asynchronously
    let tables: Vec<crate::table_extractor::Table> = Vec::new();
    // let tables = table_extractor::extract_tables_from_page(pdf_path, page_num)
    //     .unwrap_or_else(|e| {
    //         #[cfg(debug_assertions)]
    //         eprintln!("Failed to extract tables: {}", e);
    //         Vec::new()
    //     });
    
    // Extract text from blocks and place in grid using spatial coordinates
    if let Some(page) = parsed_doc.pages.first() {
        let page_id = page.id;
        let page_width = page.width;
        let page_height = page.height;
        
        #[cfg(debug_assertions)]
        eprintln!("PDF Page dimensions: {}x{}", page_width, page_height);
        
        // Collect and sort blocks by vertical position for consistent reading order
        let mut positioned_blocks: Vec<&ferrules_core::blocks::Block> = Vec::new();
        for block in &parsed_doc.blocks {
            if block.pages_id.contains(&page_id) {
                positioned_blocks.push(block);
                
                // Debug: Show what types of blocks ferrules is finding
                #[cfg(debug_assertions)]
                {
                    let block_type = match &block.kind {
                        BlockType::Title(_) => "Title",
                        BlockType::Header(_) => "Header",
                        BlockType::TextBlock(_) => "TextBlock",
                        BlockType::ListBlock(_) => "ListBlock",
                        BlockType::Table => "TABLE FOUND!",
                        BlockType::Footer(_) => "Footer",
                        BlockType::Image(_) => "Image",
                    };
                    eprintln!("Found block type: {} at y={}", block_type, block.bbox.y0);
                }
            }
        }
        positioned_blocks.sort_by(|a, b| a.bbox.y0.partial_cmp(&b.bbox.y0).unwrap_or(std::cmp::Ordering::Equal));
        
        // First pass: find min and max Y coordinates to understand the range
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for block in &positioned_blocks {
            min_y = min_y.min(block.bbox.y0);
            max_y = max_y.max(block.bbox.y1);
        }
        
        #[cfg(debug_assertions)]
        eprintln!("Y coordinate range: {} to {} (page height: {})", min_y, max_y, page_height);
        
        for block in positioned_blocks {
            // Get bounding box coordinates
            let bbox = &block.bbox;
            
            // Map PDF coordinates to character grid coordinates
            // For X: Use a small left margin instead of exact PDF position to prevent excessive indentation
            // Most text should start near the left edge for readability
            let grid_x = if bbox.x0 < page_width * 0.2 {
                2  // Small left margin for normal text
            } else if bbox.x0 > page_width * 0.5 {
                10 // Slightly more indent for clearly indented content
            } else {
                5  // Medium indent for somewhat indented content
            };
            
            // For Y: Map proportionally with padding to avoid cutting off top/bottom
            // Reserve space at top and bottom for headers/footers
            let padding_top = 1;  // Reduced to 1 line
            let padding_bottom = 1;  // Reduced to 1 line
            let usable_height = height.saturating_sub(padding_top + padding_bottom);
            
            // Normalize Y based on actual content range, not page height
            // This ensures we use the full available space
            let y_range = max_y - min_y;
            let normalized_y = if y_range > 0.0 {
                (bbox.y0 - min_y) / y_range
            } else {
                0.0
            };
            let grid_y = padding_top + ((normalized_y * usable_height as f32) as usize);
            
            // Skip if position is out of bounds
            if grid_y >= height.saturating_sub(padding_bottom) {
                #[cfg(debug_assertions)]
                eprintln!("Skipping block at y={} (exceeds height {})", grid_y, height.saturating_sub(padding_bottom));
                continue;
            }
            
            #[cfg(debug_assertions)]
            {
                let preview = match &block.kind {
                    BlockType::Title(t) => format!("Title: {}", &t.text[..t.text.len().min(50)]),
                    BlockType::Header(h) => format!("Header: {}", &h.text[..h.text.len().min(50)]),
                    BlockType::TextBlock(tb) => format!("Text: {}", &tb.text[..tb.text.len().min(50)]),
                    BlockType::Table => "Table: [TABLE DATA]".to_string(),
                    _ => "Other".to_string(),
                };
                eprintln!("Placing at grid y={}: {}", grid_y, preview);
            }
            
            // Extract text based on block type and add markdown syntax
            match &block.kind {
                BlockType::Title(t) => {
                    // Add # for title (H1)
                    let markdown_text = format!("# {}", t.text);
                    place_text_on_grid_spatial(&mut grid, &markdown_text, grid_x, grid_y, width, height);
                }
                BlockType::Header(h) => {
                    // Add ## for header (H2)
                    let markdown_text = format!("## {}", h.text);
                    place_text_on_grid_spatial(&mut grid, &markdown_text, grid_x, grid_y, width, height);
                }
                BlockType::TextBlock(tb) => {
                    // Check if this looks like table data (has multiple numbers/dollar signs)
                    let text = &tb.text;
                    let has_dollars = text.matches('$').count() > 2;
                    let has_many_numbers = text.chars().filter(|c| c.is_numeric()).count() > 10;
                    let looks_like_table = has_dollars || has_many_numbers;
                    
                    if looks_like_table {
                        // Try to format as table-like content
                        let formatted = format!("[TABLE DATA]: {}", text);
                        place_text_on_grid_spatial(&mut grid, &formatted, grid_x, grid_y, width, height);
                    } else {
                        // Regular paragraph - no prefix
                        place_text_on_grid_spatial(&mut grid, text, grid_x, grid_y, width, height);
                    }
                }
                BlockType::ListBlock(l) => {
                    // Place list items with bullet points
                    let mut list_y = grid_y;
                    for item in &l.items {
                        if list_y < height {
                            let markdown_item = format!("- {}", item);
                            place_text_on_grid_spatial(&mut grid, &markdown_item, grid_x, list_y, width, height);
                            list_y += 1;
                        }
                    }
                }
                BlockType::Table => {
                    // Check if we have a matching table from PDFium extraction
                    let mut table_found = false;
                    
                    // Find table that matches this position
                    for table in &tables {
                        // Check if table position roughly matches block position
                        let y_match = (table.y - bbox.y0).abs() < 20.0;
                        let x_match = (table.x - bbox.x0).abs() < 50.0;
                        
                        if y_match && x_match {
                            // Format table for grid display
                            let table_lines = table_extractor::table_to_grid(table, width - grid_x - 2);
                            
                            let mut current_y = grid_y;
                            for line in &table_lines {
                                if current_y < height {
                                    place_text_on_grid_spatial(&mut grid, line, grid_x, current_y, width, height);
                                    current_y += 1;
                                }
                            }
                            table_found = true;
                            break;
                        }
                    }
                    
                    if !table_found {
                        // No matching table found, use placeholder
                        let table_marker = "[TABLE DETECTED - extraction in progress]";
                        place_text_on_grid_spatial(&mut grid, table_marker, grid_x, grid_y, width, height);
                    }
                }
                BlockType::Footer(f) => {
                    // Add horizontal rule and italics for footer
                    let markdown_text = format!("---\n*{}*", f.text);
                    place_text_on_grid_spatial(&mut grid, &markdown_text, grid_x, grid_y, width, height);
                }
                _ => continue,
            }
        }
    }
    
    Ok(grid)
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
    
    // Define a reasonable right margin to prevent text running off screen
    let right_margin = 2;
    let effective_max_width = if max_width > right_margin { 
        max_width - right_margin 
    } else { 
        max_width 
    };
    
    for ch in text.chars() {
        // Handle newlines - move to next line
        if ch == '\n' {
            y += 1;
            x = x_start;
            if y >= max_height {
                break;
            }
            continue;
        }
        
        // Check if we need to wrap to next line
        if x >= effective_max_width {
            // Don't break in the middle of a word if possible
            // Look back to find the last space for word wrapping
            let mut wrap_pos = x;
            let mut found_space = false;
            
            // Search backwards from current position for a space
            for back_x in (x_start..x).rev() {
                if back_x < grid[y].len() && grid[y][back_x] == ' ' {
                    wrap_pos = back_x + 1;
                    found_space = true;
                    break;
                }
            }
            
            // Move to next line
            y += 1;
            if y >= max_height {
                break;
            }
            
            // If we found a word boundary, clear the wrapped portion on the previous line
            if found_space && y > 0 {
                for clear_x in wrap_pos..x.min(max_width) {
                    if clear_x < grid[y-1].len() {
                        grid[y-1][clear_x] = ' ';
                    }
                }
            }
            
            x = x_start;
        }
        
        // Place character on grid if within bounds
        if x < max_width && y < max_height {
            grid[y][x] = ch;
            x += 1;
        }
    }
}

pub async fn extract_to_matrix_sophisticated(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
    _use_vision: bool,
) -> Result<Vec<Vec<char>>> {
    // For now, uses the same extraction logic with markdown syntax
    // Future: could add more sophisticated spatial layout analysis
    extract_to_matrix(pdf_path, page_num, width, height).await
}

pub async fn get_markdown_content(pdf_path: &Path, page_num: usize) -> Result<String> {
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
    
    // TEMPORARILY DISABLED: Table extraction is too slow and causes freezing
    // TODO: Optimize table extraction or run it asynchronously
    let tables: Vec<crate::table_extractor::Table> = Vec::new();
    // let tables = table_extractor::extract_tables_from_page(pdf_path, page_num)
    //     .unwrap_or_else(|e| {
    //         #[cfg(debug_assertions)]
    //         eprintln!("Failed to extract tables for markdown: {}", e);
    //         Vec::new()
    //     });
    
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
                    // Look for matching table from PDFium extraction
                    let mut table_found = false;
                    
                    for table in &tables {
                        // Check if table position roughly matches block position
                        let y_match = (table.y - block.bbox.y0).abs() < 20.0;
                        let x_match = (table.x - block.bbox.x0).abs() < 50.0;
                        
                        if y_match && x_match {
                            // Convert table to markdown format
                            let table_md = table_extractor::table_to_markdown(table);
                            markdown.push_str("\n");
                            markdown.push_str(&table_md);
                            markdown.push_str("\n");
                            table_found = true;
                            break;
                        }
                    }
                    
                    if !table_found {
                        // No matching table, use placeholder
                        markdown.push_str("\n```\n[TABLE: Data table detected]\n");
                        markdown.push_str("[Extracting table structure...]\n```\n\n");
                    }
                }
                BlockType::TextBlock(tb) => {
                    let text = tb.text.trim();
                    
                    // Detect and format different text patterns
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