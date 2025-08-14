use anyhow::Result;
use ferrules_core::{
    blocks::BlockType,
    FerrulesParseConfig, FerrulesParser,
    layout::model::ORTConfig,
};
use std::path::Path;
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
    
    // Extract text from blocks and place in grid using spatial coordinates
    if let Some(page) = parsed_doc.pages.first() {
        let page_id = page.id;
        let page_width = page.width;
        let page_height = page.height;
        
        for block in &parsed_doc.blocks {
            if !block.pages_id.contains(&page_id) {
                continue;
            }
            
            // Get bounding box coordinates
            let bbox = &block.bbox;
            
            // Map PDF coordinates to character grid coordinates
            // PDF coordinates: x0, y0 (top-left), x1, y1 (bottom-right)
            // Grid coordinates: 0,0 is top-left
            let grid_x = ((bbox.x0 / page_width) * width as f32) as usize;
            let grid_y = ((bbox.y0 / page_height) * height as f32) as usize;
            
            // Skip if position is out of bounds
            if grid_x >= width || grid_y >= height {
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
                    // Regular paragraph - no prefix
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
        // Handle newlines - move to next line
        if ch == '\n' {
            y += 1;
            x = x_start;
            if y >= max_height {
                break;
            }
            continue;
        }
        
        // Skip if we're out of bounds
        if x >= max_width || y >= max_height {
            // If we hit the right edge, try wrapping to next line
            if x >= max_width && y + 1 < max_height {
                y += 1;
                x = x_start;
            } else {
                break;
            }
        }
        
        // Place character on grid
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
    
    // Convert parsed document blocks to markdown
    let mut markdown = String::new();
    
    if let Some(page) = parsed_doc.pages.first() {
        let page_id = page.id;
        
        for block in &parsed_doc.blocks {
            if !block.pages_id.contains(&page_id) {
                continue;
            }
            
            match &block.kind {
                BlockType::Title(t) => {
                    // Large title - will render differently in MARKDOWN mode
                    markdown.push_str(&format!("# {}\n\n", t.text));
                }
                BlockType::Header(h) => {
                    // Section header - will be colored in MARKDOWN mode
                    markdown.push_str(&format!("## {}\n\n", h.text));
                }
                BlockType::TextBlock(tb) => {
                    // Regular paragraph text
                    // Could enhance by detecting code patterns and wrapping in backticks
                    let text = &tb.text;
                    // Simple heuristic: if line starts with spaces/tabs, might be code
                    if text.lines().any(|line| line.starts_with("    ") || line.starts_with("\t")) {
                        markdown.push_str("```\n");
                        markdown.push_str(text);
                        markdown.push_str("\n```\n\n");
                    } else {
                        markdown.push_str(&format!("{}\n\n", text));
                    }
                }
                BlockType::ListBlock(l) => {
                    // Bulleted list - will have colored bullets in MARKDOWN mode
                    for item in &l.items {
                        markdown.push_str(&format!("- {}\n", item));
                    }
                    markdown.push_str("\n");
                }
                // Note: TableBlock type not available in current ferrules version
                // Could add table support when ferrules provides it
                BlockType::Footer(f) => {
                    // Footer with horizontal rule and italics
                    markdown.push_str(&format!("---\n\n*{}*\n\n", f.text));
                }
                _ => {}
            }
        }
    }
    
    if markdown.is_empty() {
        markdown = "# No Content\n\nNo text content found on this page.".to_string();
    }
    
    Ok(markdown)
}