use anyhow::Result;
use viuer::{Config, print};
use std::io::{self, Write};
use image::GenericImageView;

/// Display a PDF page image using viuer with optional dark mode
/// 
/// This replaces the custom kitty_graphics implementation with viuer,
/// providing better cross-terminal compatibility.
pub fn display_pdf_image(
    image: &image::DynamicImage,
    x: u16,
    y: u16,
    max_width: u16,
    max_height: u16,
    dark_mode: bool,
) -> Result<()> {
    // Save cursor position for split view consistency
    print!("\x1b[s");
    io::stdout().flush()?;
    
    // Configure viuer display settings
    let config = Config {
        // Enable transparency for PDFs with transparent backgrounds
        transparent: true,
        
        // Use absolute positioning from top-left corner
        absolute_offset: true,
        
        // Position in terminal
        x,
        y: y as i16,
        
        // Don't restore cursor - we handle that manually
        restore_cursor: false,
        
        // Set maximum dimensions - viuer will maintain aspect ratio
        width: Some(max_width as u32),
        height: Some(max_height as u32),
        
        // Use true color when available
        truecolor: true,
        
        // Use Kitty protocol if available
        use_kitty: true,
        
        // Use iTerm protocol if available
        use_iterm: true,
    };
    
    // Use image 0.25 directly (no conversion needed)
    let mut rgba = image.to_rgba8();

    // Apply dark mode filter if enabled
    if dark_mode {
        // Invert colors for dark mode
        for pixel in rgba.pixels_mut() {
            // Invert RGB but preserve alpha
            pixel[0] = 255 - pixel[0]; // R
            pixel[1] = 255 - pixel[1]; // G
            pixel[2] = 255 - pixel[2]; // B
            // pixel[3] stays the same (alpha)
        }
    }

    // Convert back to image 0.24 for viuer compatibility
    let (width, height) = (rgba.width(), rgba.height());
    let raw_buffer = rgba.into_raw();
    let old_image = image_0_24::ImageBuffer::from_raw(width, height, raw_buffer)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
    let old_dynamic = image_0_24::DynamicImage::ImageRgba8(old_image);

    // Display the image using viuer's automatic protocol detection with timeout protection
    let print_result = std::panic::catch_unwind(|| {
        print(&old_dynamic, &config)
    });

    match print_result {
        Ok(Ok(_)) => {}, // Success
        Ok(Err(e)) => return Err(anyhow::anyhow!("Viuer display error: {}", e)),
        Err(_) => return Err(anyhow::anyhow!("Viuer display panicked")),
    }
    
    // Restore cursor position
    print!("\x1b[u");
    io::stdout().flush()?;

    Ok(())
}

/// Display a portion of a PDF image based on viewport and scroll position
/// This shows the image at full size but only displays the visible portion
pub fn display_pdf_viewport(
    image: &image::DynamicImage,
    viewport_x: u16,      // Terminal position where viewport starts
    viewport_y: u16,      // Terminal position where viewport starts
    viewport_width: u16,  // Width of viewport in terminal
    viewport_height: u16, // Height of viewport in terminal
    scroll_x: u16,        // Horizontal scroll position in image (in pixels)
    scroll_y: u16,        // Vertical scroll position in image (in pixels)
    _dark_mode: bool,
) -> Result<()> {
    // For terminal display, we need to consider that each terminal cell represents multiple pixels
    // Typical terminal cell is about 7x14 pixels
    let cell_width = 7;
    let cell_height = 14;

    // Convert scroll positions from pixels to approximate cell positions
    let scroll_x_cells = scroll_x / cell_width;
    let scroll_y_cells = scroll_y / cell_height;

    // Get image dimensions
    let (img_width, img_height) = image.dimensions();

    // Calculate the pixel region to extract based on viewport and cell size
    let crop_x = (scroll_x_cells * cell_width).min(img_width.saturating_sub(1) as u16) as u32;
    let crop_y = (scroll_y_cells * cell_height).min(img_height.saturating_sub(1) as u16) as u32;
    let crop_width = (viewport_width * cell_width).min((img_width - crop_x) as u16) as u32;
    let crop_height = (viewport_height * cell_height).min((img_height - crop_y) as u16) as u32;

    // If the crop dimensions are invalid, just display the whole image scaled
    if crop_width == 0 || crop_height == 0 || crop_x >= img_width || crop_y >= img_height {
        // Fallback to displaying the whole image
        return display_pdf_image(image, viewport_x, viewport_y, viewport_width, viewport_height, _dark_mode);
    }

    // Crop the image to show only the visible portion
    let cropped = image.crop_imm(crop_x, crop_y, crop_width, crop_height);

    // Save cursor position
    print!("\x1b[s");
    io::stdout().flush()?;

    // Configure viuer to display the cropped portion
    let config = Config {
        transparent: true,
        absolute_offset: true,
        x: viewport_x,
        y: viewport_y as i16,
        restore_cursor: false,
        // Set to viewport dimensions to fill the available space
        width: Some(viewport_width as u32),
        height: Some(viewport_height as u32),
        truecolor: true,
        use_kitty: true,
        use_iterm: true,
    };

    // Convert to image 0.24 for viuer compatibility
    let rgba_image = cropped.to_rgba8();
    let (width, height) = (rgba_image.width(), rgba_image.height());
    let raw = rgba_image.into_raw();

    let old_image = image_0_24::ImageBuffer::from_raw(width, height, raw)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
    let old_dynamic = image_0_24::DynamicImage::ImageRgba8(old_image);

    // Display the cropped portion
    let print_result = std::panic::catch_unwind(|| {
        print(&old_dynamic, &config)
    });

    match print_result {
        Ok(Ok(_)) => {}, // Success
        Ok(Err(e)) => return Err(anyhow::anyhow!("Viuer display error: {}", e)),
        Err(_) => return Err(anyhow::anyhow!("Viuer display panicked")),
    }

    // Restore cursor position
    print!("\x1b[u");
    io::stdout().flush()?;

    Ok(())
}

/// Clear any displayed graphics
/// 
/// Note: Viuer doesn't provide a direct clear function, but we can
/// work around this by printing an empty/transparent image or
/// relying on terminal clear commands.
pub fn clear_graphics() -> Result<()> {
    // For Kitty protocol, send the clear command directly
    if std::env::var("KITTY_WINDOW_ID").is_ok() || 
       std::env::var("TERM_PROGRAM").unwrap_or_default() == "ghostty" {
        print!("\x1b_Ga=d\x1b\\");
        io::stdout().flush()?;
    }
    
    // For iTerm2, use its clear sequence
    if std::env::var("TERM_PROGRAM").unwrap_or_default() == "iTerm.app" {
        // iTerm2 clear inline images
        print!("\x1b]1337;File=inline=0:\x07");
        io::stdout().flush()?;
    }
    
    // Always clear the area for block mode fallback
    print!("\x1b[2J");
    io::stdout().flush()?;
    
    Ok(())
}

