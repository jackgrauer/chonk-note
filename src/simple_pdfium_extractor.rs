// Simple PDFium-based text extractor to replace Ferrules ML functionality
// Save as: src/simple_pdfium_extractor.rs

use anyhow::Result;
use pdfium_render::prelude::*;
use std::collections::BTreeMap;

/// Character with position and style information
#[derive(Debug, Clone)]
pub struct CharInfo {
    pub char: char,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub font_name: String,
}

/// Extracted text with layout preservation
#[derive(Debug)]
pub struct ExtractedContent {
    pub text: String,
    pub lines: Vec<String>,
    pub chars: Vec<CharInfo>,
}

/// Simple PDFium extractor that works without ML
pub struct SimplePdfExtractor {
    pdfium: Pdfium,
}

impl SimplePdfExtractor {
    /// Create new extractor instance
    pub fn new() -> Result<Self> {
        // Try to bind to system PDFium or use bundled version
        let pdfium = match Pdfium::new(
            Pdfium::bind_to_system_library()
                .or_else(|_| Pdfium::bind_to_library("./lib/libpdfium.dylib"))?,
        ) {
            Ok(pdfium) => pdfium,
            Err(e) => {
                eprintln!("Warning: Could not load PDFium library: {}", e);
                eprintln!("Falling back to minimal extraction");
                
                // Create minimal fallback
                Pdfium::new(
                    Pdfium::bind_to_library(
                        Pdfium::pdfium_platform_library_name_at_path("./lib/")
                    )?
                )?
            }
        };
        
        Ok(Self { pdfium })
    }
    
    /// Extract text from a specific page
    pub fn extract_page(&self, pdf_path: &str, page_num: usize) -> Result<ExtractedContent> {
        // Load the PDF document
        let document = self.pdfium.load_pdf_from_file(pdf_path, None)?;
        
        // Get the specific page
        let page = document
            .pages()
            .get(page_num)
            .ok_or_else(|| anyhow::anyhow!("Page {} not found", page_num))?;
        
        // Get page dimensions
        let width = page.width().value;
        let height = page.height().value;
        
        // Extract text with coordinates
        let mut chars = Vec::new();
        let mut lines_map: BTreeMap<i32, Vec<(f32, char)>> = BTreeMap::new();
        
        // Get all text on the page
        let text = page.text()?;
        
        // Process each character
        for segment in text.segments() {
            for char_info in segment.chars() {
                let bounds = char_info.loose_bounds()?;
                
                // Get character details
                let ch = char_info
                    .unicode()
                    .and_then(|s| s.chars().next())
                    .unwrap_or(' ');
                
                let x = bounds.left.value;
                let y = height - bounds.top.value; // Convert to top-down coordinates
                let font_size = char_info.font_size();
                
                chars.push(CharInfo {
                    char: ch,
                    x,
                    y,
                    font_size,
                    font_name: String::new(), // Font name extraction is complex, skip for now
                });
                
                // Group by approximate line (y-coordinate with tolerance)
                let line_key = (y / 10.0) as i32;
                lines_map
                    .entry(line_key)
                    .or_insert_with(Vec::new)
                    .push((x, ch));
            }
        }
        
        // Sort characters in each line by x-coordinate and build line strings
        let mut lines = Vec::new();
        for (_, mut line_chars) in lines_map {
            line_chars.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let line: String = line_chars.into_iter().map(|(_, ch)| ch).collect();
            if !line.trim().is_empty() {
                lines.push(line);
            }
        }
        
        // Build full text
        let text = lines.join("\n");
        
        Ok(ExtractedContent {
            text,
            lines,
            chars,
        })
    }
    
    /// Extract all pages from a PDF
    pub fn extract_all(&self, pdf_path: &str) -> Result<Vec<ExtractedContent>> {
        let document = self.pdfium.load_pdf_from_file(pdf_path, None)?;
        let page_count = document.pages().len();
        
        let mut results = Vec::new();
        for i in 0..page_count {
            match self.extract_page(pdf_path, i) {
                Ok(content) => results.push(content),
                Err(e) => eprintln!("Warning: Failed to extract page {}: {}", i, e),
            }
        }
        
        Ok(results)
    }
    
    /// Extract with basic table detection (no ML required)
    pub fn extract_with_tables(&self, pdf_path: &str, page_num: usize) -> Result<ExtractedContent> {
        let mut content = self.extract_page(pdf_path, page_num)?;
        
        // Simple table detection based on character alignment
        let tolerance = 5.0; // pixels
        let mut x_positions: Vec<f32> = content
            .chars
            .iter()
            .map(|c| c.x)
            .collect();
        
        x_positions.sort_by(|a, b| a.partial_cmp(b).unwrap());
        x_positions.dedup_by(|a, b| (a - b).abs() < tolerance);
        
        // If we have regular column positions, might be a table
        if x_positions.len() > 3 {
            // Add table markers to the text
            let mut enhanced_lines = Vec::new();
            for line in &content.lines {
                if line.contains("  ") || line.contains("\t") {
                    enhanced_lines.push(format!("[TABLE] {}", line));
                } else {
                    enhanced_lines.push(line.clone());
                }
            }
            content.lines = enhanced_lines;
            content.text = content.lines.join("\n");
        }
        
        Ok(content)
    }
}

// Integration helper for chonker8
pub fn extract_for_chonker(pdf_path: &str, page: usize) -> Result<String> {
    let extractor = SimplePdfExtractor::new()?;
    let content = extractor.extract_with_tables(pdf_path, page)?;
    Ok(content.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extractor_creation() {
        let extractor = SimplePdfExtractor::new();
        assert!(extractor.is_ok());
    }
}
