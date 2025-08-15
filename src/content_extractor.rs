use anyhow::Result;
use ferrules_core::{
    blocks::BlockType,
    FerrulesParseConfig, FerrulesParser,
    layout::model::ORTConfig,
};
use std::path::Path;

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
    
    // Table extraction disabled - was causing crashes
    
    // Extract text from blocks and place in grid using spatial coordinates
    if let Some(page) = parsed_doc.pages.first() {
        let page_id = page.id;
        let page_width = page.width;
        
        // Collect and sort blocks by vertical position for consistent reading order
        let mut positioned_blocks: Vec<&ferrules_core::blocks::Block> = Vec::new();
        for block in &parsed_doc.blocks {
            if block.pages_id.contains(&page_id) {
                positioned_blocks.push(block);
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
        
        for block in positioned_blocks {
            // Get bounding box coordinates
            let bbox = &block.bbox;
            
            // Map PDF coordinates to character grid coordinates
            // For X: Start most text at the left edge with minimal margin
            // This prevents text from being cut off at the edges
            let grid_x = if bbox.x0 < page_width * 0.15 {
                0  // No margin for left-aligned text to prevent cutoff
            } else if bbox.x0 > page_width * 0.6 {
                8  // Some indent for clearly indented content
            } else {
                3  // Small indent for somewhat indented content
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
                continue;
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
                    // Just show the text as-is, no table detection since it's not working
                    place_text_on_grid_spatial(&mut grid, &tb.text, grid_x, grid_y, width, height);
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
                    // Ferrules says there's a table here but doesn't give us the data
                    // Just put a simple marker
                    place_text_on_grid_spatial(&mut grid, "[Table content not extracted]", grid_x, grid_y, width, height);
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