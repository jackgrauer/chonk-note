use anyhow::Result;
use viuer::{Config, print, KittySupport};
use std::io::{self, Write};

/// Display a PDF page image using viuer
/// 
/// This replaces the custom kitty_graphics implementation with viuer,
/// providing better cross-terminal compatibility.
pub fn display_pdf_image(
    image: &image::DynamicImage,
    x: u16,
    y: u16,
    max_width: u16,
    max_height: u16,
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
    let rgba = image.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    
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

/// Get information about the detected image protocol
pub fn get_protocol_info() -> String {
    // Check for iTerm2 first
    if viuer::is_iterm_supported() {
        return "iTerm2".to_string();
    }
    
    // Check for Kitty
    match viuer::get_kitty_support() {
        KittySupport::Local => return "Kitty (local)".to_string(),
        KittySupport::Remote => return "Kitty (remote)".to_string(),
        KittySupport::None => {}
    }
    
    // Fallback to blocks
    "Blocks".to_string()
}

/// Check terminal capabilities for user feedback
pub fn check_terminal_capabilities() -> String {
    let mut capabilities = Vec::new();
    
    if viuer::is_iterm_supported() {
        capabilities.push("iTerm2");
    }
    
    match viuer::get_kitty_support() {
        KittySupport::Local => capabilities.push("Kitty (local)"),
        KittySupport::Remote => capabilities.push("Kitty (remote)"),
        KittySupport::None => {}
    }
    
    if capabilities.is_empty() {
        "Unicode blocks (fallback)".to_string()
    } else {
        capabilities.join(", ")
    }
}

/// Get optimal render size based on detected terminal protocol
pub fn get_optimal_render_size(terminal_width: u16, terminal_height: u16) -> (u32, u32) {
    let multiplier = match get_protocol_info().as_str() {
        "Kitty (local)" | "Kitty (remote)" => 3,  // Kitty handles high-res well
        "iTerm2" => 2,                            // iTerm2 also handles high-res
        _ => 1,                                    // Blocks mode - keep it smaller
    };
    
    (terminal_width as u32 * multiplier, terminal_height as u32 * multiplier)
}

/// Force clear graphics with multiple methods
pub fn force_clear_graphics() -> Result<()> {
    // Try multiple clear methods
    print!("\x1b_Ga=d\x1b\\"); // Kitty
    print!("\x1b]1337;File=inline=0:\x07"); // iTerm2
    print!("\x1b[2J"); // Clear screen
    io::stdout().flush()?;
    Ok(())
}