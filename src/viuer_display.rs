use anyhow::Result;
use viuer::{Config, print};
use std::io::{self, Write};

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
    
    // Convert from image 0.25 to image 0.24 for viuer
    // This is a bit hacky but necessary due to version mismatch
    let mut rgba = image.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    
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
    
    // Create an image 0.24 DynamicImage from raw bytes
    let raw_buffer = rgba.into_raw();
    let old_image = image_0_24::ImageBuffer::from_raw(width, height, raw_buffer)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
    let old_dynamic = image_0_24::DynamicImage::ImageRgba8(old_image);
    
    // Display the image using viuer's automatic protocol detection
    let _ = print(&old_dynamic, &config)?;
    
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

