// Simple test to check if Kitty graphics protocol works with viuer
use viuer::{Config, print};
use image::{RgbImage, DynamicImage};
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Kitty graphics protocol...");
    println!("TERM: {}", std::env::var("TERM").unwrap_or_default());
    println!("TERM_PROGRAM: {}", std::env::var("TERM_PROGRAM").unwrap_or_default());
    println!("KITTY_WINDOW_ID: {}", std::env::var("KITTY_WINDOW_ID").unwrap_or_default());
    
    // Create a simple test image (red square)
    let mut image = RgbImage::new(100, 100);
    for pixel in image.pixels_mut() {
        *pixel = image::Rgb([255, 0, 0]); // Red
    }
    let dynamic_image = DynamicImage::ImageRgb8(image);
    
    // Configure viuer to use Kitty protocol
    let config = Config {
        transparent: false,
        absolute_offset: false,
        x: 0,
        y: 0,
        restore_cursor: true,
        width: Some(50),
        height: Some(25),
        truecolor: true,
        use_kitty: true,
        use_iterm: false,
    };
    
    println!("Attempting to display red square using viuer...");
    
    match print(&dynamic_image, &config) {
        Ok(_) => {
            println!("✅ Success! Kitty graphics protocol is working!");
            println!("You should see a red square above this text.");
        },
        Err(e) => {
            println!("❌ Failed to display image: {}", e);
            println!("Viuer may have fallen back to text mode or failed completely.");
        }
    }
    
    Ok(())
}