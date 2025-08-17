// SPATIALLY ACCURATE PDF TEXT EXTRACTION
use anyhow::Result;
use pdfium_render::prelude::*;
use std::path::Path;

pub async fn extract_to_matrix(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Vec<char>>> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./lib/"))?
    );
    
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;
    
    // Create empty grid
    let mut grid = vec![vec![' '; width]; height];
    
    // Get all text from page
    let text_page = page.text()?;
    
    // Get character positions for spatial mapping
    let chars = text_page.chars();
    let page_width = page.width().value;
    let page_height = page.height().value;
    
    // Collect all characters with their positions
    let mut char_positions = Vec::new();
    for char_info in chars.iter() {
        if let Ok(bounds) = char_info.loose_bounds() {
            if let Some(ch) = char_info.unicode_string() {
                if let Some(first_char) = ch.chars().next() {
                    let x = bounds.left().value;
                    let y = bounds.top().value;
                    char_positions.push((first_char, x, y));
                }
            }
        }
    }
    
    // Sort by y position (top to bottom), then x position (left to right)
    char_positions.sort_by(|a, b| {
        let y_cmp = a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal);
        if y_cmp == std::cmp::Ordering::Equal {
            a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            y_cmp
        }
    });
    
    // Place characters on the grid with better spacing
    for (ch, x, y) in char_positions {
        if ch != ' ' && ch != '\n' && ch != '\r' {
            let grid_x = ((x / page_width) * width as f32).round() as usize;
            let grid_y = ((y / page_height) * height as f32).round() as usize;
            
            if grid_x < width && grid_y < height {
                grid[grid_y][grid_x] = ch;
            }
        }
    }
    
    Ok(grid)
}

pub async fn extract_with_ml(_pdf_path: &Path, _page_num: usize, width: usize, height: usize) -> Result<Vec<Vec<char>>> {
    // ML removed - just return empty grid
    Ok(vec![vec![' '; width]; height])
}

pub fn get_page_count(pdf_path: &Path) -> Result<usize> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./lib/"))?
    );
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    Ok(document.pages().len() as usize)
}