// Advanced PDFium spatial text extraction module
// Implements character-level coordinate extraction with full metadata
// Based on research from pdfium-render patterns and spatial layout preservation techniques

use anyhow::Result;
use pdfium_render::prelude::*;
use std::path::Path;
use std::collections::HashMap;

/// Comprehensive character data with spatial and visual metadata
#[derive(Debug, Clone)]
pub struct CharacterData {
    pub unicode: char,
    pub loose_bounds: PdfRect,      // Bounding box with padding
    pub tight_bounds: PdfRect,      // Exact glyph boundaries
    pub font_name: String,
    pub font_size: f32,
    pub font_weight: u32,
    pub font_flags: u32,            // Italic, monospace, etc.
    pub rotation: f32,              // Character rotation angle
    pub color: (u8, u8, u8),        // RGB color
    pub page_position: (f32, f32),  // Absolute position on page
    pub baseline: f32,              // Text baseline position
    pub char_width: f32,            // Character advance width
    pub object_index: usize,        // Source text object index
}

/// Page-level extraction data
pub struct PageExtractionData {
    pub characters: Vec<CharacterData>,
    pub page_width: f32,
    pub page_height: f32,
    pub text_objects: Vec<TextObjectData>,
}

/// Text object metadata for understanding text flow
#[derive(Debug, Clone)]
pub struct TextObjectData {
    pub index: usize,
    pub bounds: PdfRect,
    pub transform_matrix: [f32; 6],  // PDF transformation matrix
    pub text_direction: TextDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
}

/// Main extraction function following the research patterns
pub fn extract_characters_with_coordinates(
    pdf_path: &Path,
    page_num: usize,
) -> Result<PageExtractionData> {
    // Initialize PDFium with proper library binding
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path("./lib/")
        )?
    );
    
    // Load document
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let pages = document.pages();
    let page = pages.get(page_num as u16)?;
    
    // Get page dimensions for coordinate transformation
    let page_width = page.width().value;
    let page_height = page.height().value;
    
    // Initialize text page for character extraction
    let text_page = page.text()?;
    
    let mut characters = Vec::new();
    let mut text_objects = Vec::new();
    let mut object_index = 0;
    
    // Iterate through all page objects to find text
    for object in page.objects().iter() {
        if let Some(text_object) = object.as_text_object() {
            // Extract text object metadata
            let object_bounds = text_object.bounds()?;
            let transform = extract_transform_matrix(&text_object)?;
            let text_direction = detect_text_direction(&transform);
            
            text_objects.push(TextObjectData {
                index: object_index,
                bounds: object_bounds,
                transform_matrix: transform,
                text_direction: text_direction.clone(),
            });
            
            // Get font information for this text object
            let font = text_object.font();
            let font_name = font.as_ref()
                .and_then(|f| f.name().ok())
                .unwrap_or_else(|| "Unknown".to_string());
            let font_weight = font.as_ref()
                .and_then(|f| f.weight().ok())
                .unwrap_or(400);
            let font_flags = font.as_ref()
                .map(|f| extract_font_flags(f))
                .unwrap_or(0);
            
            let font_size = text_object.scaled_font_size();
            
            // Get text color
            let color = text_object.fill_color()
                .ok()
                .and_then(|c| c.to_rgb().ok())
                .map(|rgb| (rgb.red(), rgb.green(), rgb.blue()))
                .unwrap_or((0, 0, 0));
            
            // Extract characters with their coordinates
            if let Ok(chars) = text_page.chars_for_object(&text_object) {
                for char in chars.iter() {
                    // Get both loose and tight bounds as per research
                    let loose_bounds = char.loose_bounds()?;
                    let tight_bounds = char.tight_bounds()?;
                    
                    // Calculate character position
                    let page_position = (
                        loose_bounds.left.value,
                        loose_bounds.bottom.value,
                    );
                    
                    // Get character rotation if text is rotated
                    let rotation = calculate_rotation(&transform);
                    
                    // Calculate baseline from bounds
                    let baseline = loose_bounds.bottom.value;
                    
                    // Calculate character width
                    let char_width = loose_bounds.right.value - loose_bounds.left.value;
                    
                    let char_data = CharacterData {
                        unicode: char.unicode_char()?,
                        loose_bounds,
                        tight_bounds,
                        font_name: font_name.clone(),
                        font_size,
                        font_weight,
                        font_flags,
                        rotation,
                        color,
                        page_position,
                        baseline,
                        char_width,
                        object_index,
                    };
                    
                    characters.push(char_data);
                }
            }
            
            object_index += 1;
        }
    }
    
    Ok(PageExtractionData {
        characters,
        page_width,
        page_height,
        text_objects,
    })
}

/// Extract transformation matrix from text object
fn extract_transform_matrix(text_object: &PdfPageTextObject) -> Result<[f32; 6]> {
    // PDFium provides transformation matrix for text objects
    // Default identity matrix if not available
    // [a, b, c, d, e, f] where:
    // a, d: scaling
    // b, c: rotation/skew
    // e, f: translation
    
    // This would need FFI access to FPDFText_GetMatrix or similar
    // For now, return identity matrix as placeholder
    Ok([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
}

/// Detect text direction from transformation matrix
fn detect_text_direction(matrix: &[f32; 6]) -> TextDirection {
    let [a, b, c, d, _, _] = *matrix;
    
    // Analyze rotation component
    if b.abs() > 0.1 || c.abs() > 0.1 {
        // Text is rotated
        let angle = b.atan2(a);
        
        if angle.abs() < std::f32::consts::PI / 4.0 {
            TextDirection::LeftToRight
        } else if angle.abs() > 3.0 * std::f32::consts::PI / 4.0 {
            TextDirection::RightToLeft
        } else if angle > 0.0 {
            TextDirection::BottomToTop
        } else {
            TextDirection::TopToBottom
        }
    } else if a < 0.0 {
        TextDirection::RightToLeft
    } else {
        TextDirection::LeftToRight
    }
}

/// Calculate rotation angle from transformation matrix
fn calculate_rotation(matrix: &[f32; 6]) -> f32 {
    let [a, b, _, _, _, _] = *matrix;
    b.atan2(a)
}

/// Extract font flags for style detection
fn extract_font_flags(font: &PdfFont) -> u32 {
    let mut flags = 0u32;
    
    // Check for italic (bit 0)
    if let Ok(true) = font.is_italic() {
        flags |= 1 << 0;
    }
    
    // Check for bold is already in font_weight
    
    // Check for monospace (bit 1)
    if let Ok(true) = font.is_fixed_pitch() {
        flags |= 1 << 1;
    }
    
    // Check for serif (bit 2)
    if let Ok(true) = font.is_serif() {
        flags |= 1 << 2;
    }
    
    // Check for symbolic (bit 3)
    if let Ok(true) = font.is_symbolic() {
        flags |= 1 << 3;
    }
    
    flags
}

/// Coordinate transformation utilities
pub mod coordinates {
    use super::*;
    
    /// Convert PDF coordinates (origin bottom-left) to screen space (origin top-left)
    pub fn pdf_to_screen(
        pdf_point: (f32, f32),
        page_height: f32,
    ) -> (f32, f32) {
        (pdf_point.0, page_height - pdf_point.1)
    }
    
    /// Convert screen coordinates to PDF space
    pub fn screen_to_pdf(
        screen_point: (f32, f32),
        page_height: f32,
    ) -> (f32, f32) {
        (screen_point.0, page_height - screen_point.1)
    }
    
    /// Apply rotation transformation to a point
    pub fn apply_rotation(
        point: (f32, f32),
        angle: f32,
        origin: (f32, f32),
    ) -> (f32, f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let dx = point.0 - origin.0;
        let dy = point.1 - origin.1;
        
        (
            origin.0 + dx * cos_a - dy * sin_a,
            origin.1 + dx * sin_a + dy * cos_a,
        )
    }
    
    /// Convert character coordinates to grid position
    pub fn to_grid_position(
        char_pos: (f32, f32),
        page_width: f32,
        page_height: f32,
        grid_width: usize,
        grid_height: usize,
    ) -> (usize, usize) {
        let x_ratio = char_pos.0 / page_width;
        let y_ratio = char_pos.1 / page_height;
        
        let grid_x = (x_ratio * grid_width as f32) as usize;
        let grid_y = (y_ratio * grid_height as f32) as usize;
        
        (
            grid_x.min(grid_width - 1),
            grid_y.min(grid_height - 1),
        )
    }
}

/// Character analysis utilities
pub mod analysis {
    use super::*;
    
    /// Detect if character is part of a header based on font metadata
    pub fn is_header_char(char: &CharacterData, avg_font_size: f32) -> bool {
        char.font_size > avg_font_size * 1.3 || char.font_weight > 600
    }
    
    /// Detect if character is italic
    pub fn is_italic(char: &CharacterData) -> bool {
        char.font_flags & (1 << 0) != 0
    }
    
    /// Detect if character is monospace (likely code)
    pub fn is_monospace(char: &CharacterData) -> bool {
        char.font_flags & (1 << 1) != 0
    }
    
    /// Calculate average font size for a page
    pub fn calculate_avg_font_size(chars: &[CharacterData]) -> f32 {
        if chars.is_empty() {
            return 12.0;
        }
        
        let sum: f32 = chars.iter().map(|c| c.font_size).sum();
        sum / chars.len() as f32
    }
    
    /// Group characters by their text object for understanding text flow
    pub fn group_by_text_object(
        chars: &[CharacterData],
    ) -> HashMap<usize, Vec<&CharacterData>> {
        let mut groups = HashMap::new();
        
        for char in chars {
            groups.entry(char.object_index)
                .or_insert_with(Vec::new)
                .push(char);
        }
        
        groups
    }
    
    /// Detect if a group of characters forms RTL text
    pub fn is_rtl_text(chars: &[CharacterData]) -> bool {
        // Check for Arabic, Hebrew, or other RTL Unicode ranges
        chars.iter().any(|c| {
            let code = c.unicode as u32;
            // Arabic: U+0600–U+06FF, U+0750–U+077F
            // Hebrew: U+0590–U+05FF
            (0x0600..=0x06FF).contains(&code) ||
            (0x0750..=0x077F).contains(&code) ||
            (0x0590..=0x05FF).contains(&code)
        })
    }
}

/// Spatial clustering for layout reconstruction
pub mod clustering {
    use super::*;
    
    /// Simple distance calculation between characters
    pub fn char_distance(a: &CharacterData, b: &CharacterData) -> f32 {
        let dx = a.page_position.0 - b.page_position.0;
        let dy = a.page_position.1 - b.page_position.1;
        (dx * dx + dy * dy).sqrt()
    }
    
    /// Check if two characters are on the same baseline (likely same line)
    pub fn same_baseline(a: &CharacterData, b: &CharacterData, tolerance: f32) -> bool {
        (a.baseline - b.baseline).abs() < tolerance
    }
    
    /// Check if two characters are horizontally adjacent (likely same word)
    pub fn horizontally_adjacent(
        a: &CharacterData,
        b: &CharacterData,
        max_gap: f32,
    ) -> bool {
        let gap = (b.loose_bounds.left.value - a.loose_bounds.right.value).abs();
        gap < max_gap && same_baseline(a, b, 2.0)
    }
    
    /// Simple word clustering based on proximity
    pub fn cluster_into_words(
        chars: &[CharacterData],
    ) -> Vec<Vec<CharacterData>> {
        if chars.is_empty() {
            return Vec::new();
        }
        
        let mut words = Vec::new();
        let mut current_word = vec![chars[0].clone()];
        
        // Calculate average character width for gap detection
        let avg_char_width = chars.iter()
            .map(|c| c.char_width)
            .sum::<f32>() / chars.len() as f32;
        
        let max_gap = avg_char_width * 0.3; // 30% of average char width
        
        for i in 1..chars.len() {
            if horizontally_adjacent(&chars[i-1], &chars[i], max_gap) {
                current_word.push(chars[i].clone());
            } else {
                if !current_word.is_empty() {
                    words.push(current_word);
                }
                current_word = vec![chars[i].clone()];
            }
        }
        
        if !current_word.is_empty() {
            words.push(current_word);
        }
        
        words
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_coordinate_transformation() {
        let pdf_point = (100.0, 200.0);
        let page_height = 792.0; // US Letter height in points
        
        let screen_point = coordinates::pdf_to_screen(pdf_point, page_height);
        assert_eq!(screen_point, (100.0, 592.0));
        
        let back_to_pdf = coordinates::screen_to_pdf(screen_point, page_height);
        assert_eq!(back_to_pdf, pdf_point);
    }
    
    #[test]
    fn test_rotation() {
        let point = (10.0, 0.0);
        let angle = std::f32::consts::PI / 2.0; // 90 degrees
        let origin = (0.0, 0.0);
        
        let rotated = coordinates::apply_rotation(point, angle, origin);
        assert!((rotated.0 - 0.0).abs() < 0.001);
        assert!((rotated.1 - 10.0).abs() < 0.001);
    }
}
