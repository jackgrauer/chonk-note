use anyhow::Result;
use image::DynamicImage;
use pdfium_render::prelude::*;
use std::path::Path;


/// Render a PDF page to an image
pub fn render_pdf_page(pdf_path: &Path, page_num: usize, width: u32, height: u32) -> Result<DynamicImage> {
    // Create PDFium instance using static bindings
    let pdfium = Pdfium::default();
    
    // Load the PDF document
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    
    // Get the requested page (pages are 0-indexed)
    let pages = document.pages();
    let page = pages.get(page_num as u16)?;
    
    // Calculate scale to fit within the specified dimensions
    let page_width = page.width();
    let page_height = page.height();
    
    let scale_x = width as f32 / page_width.value;
    let scale_y = height as f32 / page_height.value;
    let scale = scale_x.min(scale_y);
    
    // Render the page to a bitmap
    let bitmap = page.render_with_config(
        &PdfRenderConfig::new()
            .set_target_size(
                (page_width.value * scale) as i32,
                (page_height.value * scale) as i32
            )
            .rotate_if_landscape(PdfPageRenderRotation::None, false)
    )?;
    
    // Convert to DynamicImage
    let image = bitmap.as_image();
    Ok(image)
}

