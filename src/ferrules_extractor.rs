use anyhow::Result;
use ferrules_core::{
    blocks::BlockType,
    FerrulesParseConfig, FerrulesParser,
    layout::model::ORTConfig,
};
use std::path::Path;
use crate::pdf_to_grid::{SpatialTextGrid, PdfPoint, PdfSize};

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
            
            // Extract text based on block type
            let text = match &block.kind {
                BlockType::TextBlock(tb) => &tb.text,
                BlockType::Title(t) => &t.text,
                BlockType::Header(tb) => &tb.text,
                BlockType::Footer(tb) => &tb.text,
                BlockType::ListBlock(l) => {
                    // Place list items spatially
                    let mut list_y = grid_y;
                    for item in &l.items {
                        if list_y < height {
                            place_text_on_grid_spatial(&mut grid, item, grid_x + 2, list_y, width, height);
                            list_y += 1;
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            
            // Place text on grid at the proper spatial position
            place_text_on_grid_spatial(&mut grid, text, grid_x, grid_y, width, height);
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
    use_vision: bool,
) -> Result<Vec<Vec<char>>> {
    if use_vision {
        // Use Ferrules AI vision for extraction
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
        
        // Use sophisticated grid with Ferrules data
        if let Some(page) = parsed_doc.pages.first() {
            let page_id = page.id;
            let page_width = page.width;
            let page_height = page.height;
            
            // Create sophisticated spatial grid
            let mut grid = SpatialTextGrid::new(page_width, page_height, width, height);
            
            // Add all text blocks with their bounding boxes
            for block in &parsed_doc.blocks {
                if !block.pages_id.contains(&page_id) {
                    continue;
                }
                
                let bbox = &block.bbox;
                
                // Extract text and font size estimate based on block type
                let (text, font_size) = match &block.kind {
                    BlockType::Title(t) => (t.text.clone(), 16.0), // Titles are larger
                    BlockType::Header(tb) => (tb.text.clone(), 14.0), // Headers medium
                    BlockType::TextBlock(tb) => (tb.text.clone(), 11.0), // Normal text
                    BlockType::Footer(tb) => (tb.text.clone(), 9.0), // Footers smaller
                    BlockType::ListBlock(l) => {
                        // Add list items as separate blocks
                        for (i, item) in l.items.iter().enumerate() {
                            let item_y = bbox.y0 + (i as f32 * 12.0); // Estimate line height
                            grid.add_text_block(
                                item,
                                bbox.x0 + 10.0, // Indent list items
                                item_y,
                                bbox.width() - 10.0,
                                12.0,
                                10.0, // List item font size
                            );
                        }
                        continue;
                    }
                    _ => continue,
                };
                
                // Add the text block to the grid
                grid.add_text_block(
                    &text,
                    bbox.x0,
                    bbox.y0,
                    bbox.width(),
                    bbox.height(),
                    font_size,
                );
            }
            
            // Layout all text blocks with collision detection
            grid.layout()?;
            
            // Convert to character grid
            Ok(grid.to_char_grid())
        } else {
            // No pages found, return empty grid
            Ok(vec![vec![' '; width]; height])
        }
    } else {
        // Without vision model, just create an empty grid or use basic extraction
        // For now, return a grid with a message
        let mut grid = vec![vec![' '; width]; height];
        let msg = "Vision model disabled - enable in Options (press 'm' then 'v')";
        for (i, ch) in msg.chars().enumerate() {
            if i < width && height > 10 {
                grid[10][i] = ch;
            }
        }
        Ok(grid)
    }
}