use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use crossterm::{cursor::MoveTo, execute};
use image::{DynamicImage, ImageBuffer, Rgba};
use std::io::{self, Write};

/// Check what image protocol the terminal supports
pub fn detect_image_support() -> ImageProtocol {
    // Check for Kitty
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return ImageProtocol::Kitty;
    }
    
    // Check for Ghostty (supports Kitty protocol)
    if std::env::var("TERM_PROGRAM").unwrap_or_default() == "ghostty" {
        return ImageProtocol::Kitty;
    }
    
    // Check for iTerm2
    if std::env::var("TERM_PROGRAM").unwrap_or_default() == "iTerm.app" {
        return ImageProtocol::ITerm2;
    }
    
    // Check for WezTerm
    if std::env::var("TERM_PROGRAM").unwrap_or_default() == "WezTerm" {
        return ImageProtocol::ITerm2; // WezTerm supports iTerm2 protocol
    }
    
    // Default to block characters
    ImageProtocol::Blocks
}

#[derive(Debug, Clone, Copy)]
pub enum ImageProtocol {
    Kitty,
    ITerm2,
    Blocks,
}

/// Display an image at the specified position using the best available protocol
pub fn display_image(
    image: &DynamicImage,
    x: u16,
    y: u16,
    max_width: u16,
    max_height: u16,
) -> Result<()> {
    let protocol = detect_image_support();
    
    // Scale image to fit
    let img_width = image.width();
    let img_height = image.height();
    let scale_x = max_width as f32 / img_width as f32;
    let scale_y = max_height as f32 / img_height as f32;
    let scale = scale_x.min(scale_y).min(1.0); // Don't upscale
    
    let new_width = (img_width as f32 * scale) as u32;
    let new_height = (img_height as f32 * scale) as u32;
    
    let scaled = if scale < 1.0 {
        image.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
    } else {
        image.clone()
    };
    
    match protocol {
        ImageProtocol::Kitty => display_image_kitty(&scaled, x, y),
        ImageProtocol::ITerm2 => display_image_iterm2(&scaled, x, y),
        ImageProtocol::Blocks => display_image_blocks(&scaled, x, y, max_width, max_height),
    }
}

/// Display image using Kitty graphics protocol
fn display_image_kitty(image: &DynamicImage, x: u16, y: u16) -> Result<()> {
    let mut stdout = io::stdout();
    
    // Move cursor to position
    execute!(stdout, MoveTo(x, y))?;
    
    // Convert image to PNG bytes
    let mut png_bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)?;
    
    // Encode as base64
    let encoded = STANDARD.encode(&png_bytes);
    
    // Get dimensions
    let width = image.width();
    let height = image.height();
    
    // Send image using Kitty protocol
    // a=T: transmit and display
    // f=100: PNG format  
    // t=d: direct (not shared memory)
    // The image will be placed at the current cursor position
    write!(
        stdout,
        "\x1b_Ga=T,t=d,f=100,s={},v={};{}\x1b\\",
        width, height, encoded
    )?;
    
    stdout.flush()?;
    Ok(())
}

/// Display image using iTerm2 inline images protocol
fn display_image_iterm2(image: &DynamicImage, x: u16, y: u16) -> Result<()> {
    let mut stdout = io::stdout();
    
    // Move cursor to position
    execute!(stdout, MoveTo(x, y))?;
    
    // Convert image to PNG bytes
    let mut png_bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)?;
    
    // Encode as base64
    let encoded = STANDARD.encode(&png_bytes);
    
    // Get dimensions
    let width = image.width();
    let height = image.height();
    
    // iTerm2 protocol
    write!(
        stdout,
        "\x1b]1337;File=inline=1;width={};height={};preserveAspectRatio=1:{}\x07",
        width, height, encoded
    )?;
    
    stdout.flush()?;
    Ok(())
}

/// Display image using Unicode block characters (fallback)
fn display_image_blocks(
    image: &DynamicImage,
    start_x: u16,
    start_y: u16,
    max_width: u16,
    max_height: u16,
) -> Result<()> {
    let mut stdout = io::stdout();
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    
    // Each character represents 2x2 pixels (using quarter blocks)
    let char_width = (max_width as usize).min(width as usize / 2);
    let char_height = (max_height as usize).min(height as usize / 2);
    
    // Unicode block characters for different fill levels
    const BLOCKS: [char; 16] = [
        ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛',
        '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
    ];
    
    for cy in 0..char_height {
        execute!(stdout, MoveTo(start_x, start_y + cy as u16))?;
        
        for cx in 0..char_width {
            // Sample 2x2 pixel region
            let px = cx * 2;
            let py = cy * 2;
            
            // Get the 4 pixels
            let tl = get_pixel_brightness(&rgba, px, py);
            let tr = get_pixel_brightness(&rgba, px + 1, py);
            let bl = get_pixel_brightness(&rgba, px, py + 1);
            let br = get_pixel_brightness(&rgba, px + 1, py + 1);
            
            // Convert to block index (4 bits, one per quadrant)
            let mut index = 0;
            if tl > 128 { index |= 0b0001; }
            if tr > 128 { index |= 0b0010; }
            if bl > 128 { index |= 0b0100; }
            if br > 128 { index |= 0b1000; }
            
            // Get average color for the block
            let avg_color = get_average_color(&rgba, px, py, 2, 2);
            
            // Set color and print block
            write!(
                stdout,
                "\x1b[38;2;{};{};{}m{}",
                avg_color.0, avg_color.1, avg_color.2,
                BLOCKS[index]
            )?;
        }
        
        // Reset color at end of line
        write!(stdout, "\x1b[0m")?;
    }
    
    stdout.flush()?;
    Ok(())
}

/// Get brightness of a pixel (0-255)
fn get_pixel_brightness(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, x: usize, y: usize) -> u8 {
    if x >= img.width() as usize || y >= img.height() as usize {
        return 0;
    }
    
    let pixel = img.get_pixel(x as u32, y as u32);
    // Simple brightness calculation
    let r = pixel[0] as u32;
    let g = pixel[1] as u32;
    let b = pixel[2] as u32;
    ((r + g + b) / 3) as u8
}

/// Get average color of a region
fn get_average_color(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) -> (u8, u8, u8) {
    let mut r_sum = 0u32;
    let mut g_sum = 0u32;
    let mut b_sum = 0u32;
    let mut count = 0u32;
    
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img.width() as usize && py < img.height() as usize {
                let pixel = img.get_pixel(px as u32, py as u32);
                r_sum += pixel[0] as u32;
                g_sum += pixel[1] as u32;
                b_sum += pixel[2] as u32;
                count += 1;
            }
        }
    }
    
    if count == 0 {
        (0, 0, 0)
    } else {
        (
            (r_sum / count) as u8,
            (g_sum / count) as u8,
            (b_sum / count) as u8,
        )
    }
}

/// Clear any graphics from the terminal
pub fn clear_graphics() -> Result<()> {
    let protocol = detect_image_support();
    let mut stdout = io::stdout();
    
    match protocol {
        ImageProtocol::Kitty => {
            // Kitty: delete all images
            write!(stdout, "\x1b_Ga=d\x1b\\")?;
        }
        ImageProtocol::ITerm2 => {
            // iTerm2 doesn't have a specific clear command
            // Images are cleared when overwritten
        }
        ImageProtocol::Blocks => {
            // Block characters are cleared by normal text operations
        }
    }
    
    stdout.flush()?;
    Ok(())
}