// MINIMAL PDF TEXT EXTRACTION
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
    let chars = text_page.chars();
    
    // Map each character to grid position
    for char_info in chars.iter() {
        if let Ok(bounds) = char_info.loose_bounds() {
            let ch = char_info.unicode_string().unwrap_or_default().chars().next().unwrap_or(' ');
            
            // Map PDF coordinates to grid
            let grid_x = ((bounds.left().value / page.width().value) * width as f32) as usize;
            let grid_y = ((bounds.top().value / page.height().value) * height as f32) as usize;
            
            if grid_x < width && grid_y < height && ch != ' ' {
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