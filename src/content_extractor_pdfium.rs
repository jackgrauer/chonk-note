// Enhanced content extractor using direct PDFium character extraction
// This version replaces ferrules ML extraction with precise coordinate-based extraction

use anyhow::Result;
use std::path::Path;
use std::collections::HashMap;

// Import the new PDFium spatial extractor
mod pdfium_spatial_extractor;
use pdfium_spatial_extractor::{
    CharacterData, PageExtractionData,
    extract_characters_with_coordinates,
    coordinates, analysis, clustering,
};

// Re-export for compatibility
pub use crate::pdf_renderer::get_pdf_page_count as get_page_count;

/// Advanced spatial text extraction with layout preservation
pub async fn extract_to_matrix(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Vec<char>>> {
    // Extract character-level data with coordinates
    let extraction_data = tokio::task::spawn_blocking({
        let pdf_path = pdf_path.to_path_buf();
        move || extract_characters_with_coordinates(&pdf_path, page_num)
    }).await??;
    
    // Create spatial grid mapper
    let mut grid_mapper = SpatialGridMapper::new(
        width,
        height,
        extraction_data,
    );
    
    // Perform spatial clustering and layout analysis
    grid_mapper.analyze_layout();
    
    // Map to character grid with intelligent placement
    Ok(grid_mapper.map_to_grid())
}

/// Spatial grid mapper with clustering algorithms
struct SpatialGridMapper {
    grid_width: usize,
    grid_height: usize,
    page_data: PageExtractionData,
    word_clusters: Vec<Vec<CharacterData>>,
    line_clusters: Vec<Vec<Vec<CharacterData>>>,
    table_regions: Vec<TableRegion>,
}

#[derive(Debug)]
struct TableRegion {
    bounds: (f32, f32, f32, f32), // (x0, y0, x1, y1)
    cells: Vec<Vec<String>>,
}

impl SpatialGridMapper {
    fn new(width: usize, height: usize, page_data: PageExtractionData) -> Self {
        Self {
            grid_width: width,
            grid_height: height,
            page_data,
            word_clusters: Vec::new(),
            line_clusters: Vec::new(),
            table_regions: Vec::new(),
        }
    }
    
    /// Analyze page layout using spatial clustering
    fn analyze_layout(&mut self) {
        // Step 1: Cluster characters into words
        self.word_clusters = self.cluster_into_words();
        
        // Step 2: Cluster words into lines
        self.line_clusters = self.cluster_into_lines();
        
        // Step 3: Detect tables using coordinate alignment
        self.table_regions = self.detect_tables();
    }
    
    /// Cluster characters into words based on proximity
    fn cluster_into_words(&self) -> Vec<Vec<CharacterData>> {
        clustering::cluster_into_words(&self.page_data.characters)
    }
    
    /// Cluster words into lines based on baseline alignment
    fn cluster_into_lines(&self) -> Vec<Vec<Vec<CharacterData>>> {
        let mut lines = Vec::new();
        let mut used_words = vec![false; self.word_clusters.len()];
        
        for (i, word) in self.word_clusters.iter().enumerate() {
            if used_words[i] || word.is_empty() {
                continue;
            }
            
            let mut line = vec![word.clone()];
            used_words[i] = true;
            
            let baseline = word[0].baseline;
            let tolerance = 2.0; // pixels
            
            // Find other words on the same baseline
            for (j, other_word) in self.word_clusters.iter().enumerate() {
                if used_words[j] || other_word.is_empty() {
                    continue;
                }
                
                if (other_word[0].baseline - baseline).abs() < tolerance {
                    line.push(other_word.clone());
                    used_words[j] = true;
                }
            }
            
            // Sort words in line by x-position
            line.sort_by(|a, b| {
                a[0].page_position.0.partial_cmp(&b[0].page_position.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            
            lines.push(line);
        }
        
        // Sort lines by y-position (top to bottom in screen space)
        lines.sort_by(|a, b| {
            if a.is_empty() || a[0].is_empty() || b.is_empty() || b[0].is_empty() {
                return std::cmp::Ordering::Equal;
            }
            
            let a_y = coordinates::pdf_to_screen(
                a[0][0].page_position,
                self.page_data.page_height,
            ).1;
            let b_y = coordinates::pdf_to_screen(
                b[0][0].page_position,
                self.page_data.page_height,
            ).1;
            
            a_y.partial_cmp(&b_y).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        lines
    }
    
    /// Detect tables using coordinate alignment analysis
    fn detect_tables(&self) -> Vec<TableRegion> {
        let mut tables = Vec::new();
        
        // Group characters by x-coordinate alignment (columns)
        let column_groups = self.detect_column_alignments();
        
        // Group characters by y-coordinate alignment (rows)
        let row_groups = self.detect_row_alignments();
        
        // Find intersections to identify table regions
        if column_groups.len() >= 2 && row_groups.len() >= 2 {
            // Simple table detection: if we have multiple aligned columns and rows
            // in the same region, it's likely a table
            
            // For now, return empty - full implementation would analyze
            // intersections and reconstruct table cells
        }
        
        tables
    }
    
    /// Detect vertical alignments (potential columns)
    fn detect_column_alignments(&self) -> Vec<Vec<&CharacterData>> {
        let mut x_groups: HashMap<i32, Vec<&CharacterData>> = HashMap::new();
        let tolerance = 3.0; // pixels
        
        for char in &self.page_data.characters {
            let x_bucket = (char.page_position.0 / tolerance) as i32;
            x_groups.entry(x_bucket).or_default().push(char);
        }
        
        // Filter groups with significant vertical extent
        x_groups.into_iter()
            .filter(|(_, group)| group.len() >= 3)
            .map(|(_, group)| group)
            .collect()
    }
    
    /// Detect horizontal alignments (potential rows)
    fn detect_row_alignments(&self) -> Vec<Vec<&CharacterData>> {
        let mut y_groups: HashMap<i32, Vec<&CharacterData>> = HashMap::new();
        let tolerance = 2.0; // pixels
        
        for char in &self.page_data.characters {
            let y_bucket = (char.baseline / tolerance) as i32;
            y_groups.entry(y_bucket).or_default().push(char);
        }
        
        // Filter groups with significant horizontal extent
        y_groups.into_iter()
            .filter(|(_, group)| group.len() >= 3)
            .map(|(_, group)| group)
            .collect()
    }
    
    /// Map analyzed layout to character grid
    fn map_to_grid(&self) -> Vec<Vec<char>> {
        let mut grid = vec![vec![' '; self.grid_width]; self.grid_height];
        
        // Calculate average font size for header detection
        let avg_font_size = analysis::calculate_avg_font_size(&self.page_data.characters);
        
        // Process each line
        for (line_idx, line) in self.line_clusters.iter().enumerate() {
            if line.is_empty() || line[0].is_empty() {
                continue;
            }
            
            // Get the first character to determine line properties
            let first_char = &line[0][0];
            
            // Convert to screen coordinates and then to grid position
            let screen_pos = coordinates::pdf_to_screen(
                first_char.page_position,
                self.page_data.page_height,
            );
            
            let (_, grid_y) = coordinates::to_grid_position(
                screen_pos,
                self.page_data.page_width,
                self.page_data.page_height,
                self.grid_width,
                self.grid_height,
            );
            
            // Skip if out of bounds
            if grid_y >= self.grid_height {
                continue;
            }
            
            // Determine line formatting based on font metadata
            let is_header = analysis::is_header_char(first_char, avg_font_size);
            let is_italic = analysis::is_italic(first_char);
            let is_monospace = analysis::is_monospace(first_char);
            
            // Build line text with appropriate formatting
            let mut line_text = String::new();
            
            // Add markdown formatting based on detected style
            if is_header && first_char.font_size > avg_font_size * 1.5 {
                line_text.push_str("# "); // H1 for large headers
            } else if is_header {
                line_text.push_str("## "); // H2 for smaller headers
            } else if is_monospace {
                line_text.push_str("    "); // Code block indentation
            }
            
            // Add italic markers if needed
            if is_italic && !is_monospace {
                line_text.push('*');
            }
            
            // Concatenate words in line
            for (word_idx, word) in line.iter().enumerate() {
                if word_idx > 0 {
                    line_text.push(' ');
                }
                
                for char in word {
                    line_text.push(char.unicode);
                }
            }
            
            // Close italic markers
            if is_italic && !is_monospace {
                line_text.push('*');
            }
            
            // Determine x position based on indentation detection
            let grid_x = self.detect_indentation_level(first_char);
            
            // Place line on grid
            self.place_text_on_grid(&mut grid, &line_text, grid_x, grid_y);
        }
        
        // Render table regions separately
        for table in &self.table_regions {
            self.render_table_to_grid(&mut grid, table);
        }
        
        grid
    }
    
    /// Detect indentation level based on x-position
    fn detect_indentation_level(&self, char: &CharacterData) -> usize {
        let x_ratio = char.page_position.0 / self.page_data.page_width;
        
        if x_ratio < 0.15 {
            0  // Left-aligned
        } else if x_ratio < 0.25 {
            2  // Slight indent
        } else if x_ratio < 0.35 {
            4  // Medium indent
        } else if x_ratio > 0.6 {
            8  // Right-side content
        } else {
            6  // Heavy indent
        }
    }
    
    /// Place text on grid with word wrapping
    fn place_text_on_grid(
        &self,
        grid: &mut Vec<Vec<char>>,
        text: &str,
        x_start: usize,
        y_start: usize,
    ) {
        let mut x = x_start;
        let mut y = y_start;
        
        for ch in text.chars() {
            if ch == '\n' {
                y += 1;
                x = x_start;
                if y >= self.grid_height {
                    break;
                }
                continue;
            }
            
            // Word wrap at grid boundary
            if x >= self.grid_width {
                y += 1;
                if y >= self.grid_height {
                    break;
                }
                x = 0;
            }
            
            // Place character
            if x < self.grid_width && y < self.grid_height {
                grid[y][x] = ch;
                x += 1;
            }
        }
    }
    
    /// Render table region to grid with ASCII borders
    fn render_table_to_grid(&self, grid: &mut Vec<Vec<char>>, table: &TableRegion) {
        // Convert table bounds to grid coordinates
        let (x0, y0, x1, y1) = table.bounds;
        
        let (grid_x0, grid_y0) = coordinates::to_grid_position(
            coordinates::pdf_to_screen((x0, y0), self.page_data.page_height),
            self.page_data.page_width,
            self.page_data.page_height,
            self.grid_width,
            self.grid_height,
        );
        
        let (grid_x1, grid_y1) = coordinates::to_grid_position(
            coordinates::pdf_to_screen((x1, y1), self.page_data.page_height),
            self.page_data.page_width,
            self.page_data.page_height,
            self.grid_width,
            self.grid_height,
        );
        
        // Draw table borders
        for x in grid_x0..=grid_x1.min(self.grid_width - 1) {
            if grid_y0 < self.grid_height {
                grid[grid_y0][x] = '─';
            }
            if grid_y1 < self.grid_height {
                grid[grid_y1][x] = '─';
            }
        }
        
        for y in grid_y0..=grid_y1.min(self.grid_height - 1) {
            if grid_x0 < self.grid_width {
                grid[y][grid_x0] = '│';
            }
            if grid_x1 < self.grid_width {
                grid[y][grid_x1] = '│';
            }
        }
        
        // Draw corners
        if grid_y0 < self.grid_height && grid_x0 < self.grid_width {
            grid[grid_y0][grid_x0] = '┌';
        }
        if grid_y0 < self.grid_height && grid_x1 < self.grid_width {
            grid[grid_y0][grid_x1] = '┐';
        }
        if grid_y1 < self.grid_height && grid_x0 < self.grid_width {
            grid[grid_y1][grid_x0] = '└';
        }
        if grid_y1 < self.grid_height && grid_x1 < self.grid_width {
            grid[grid_y1][grid_x1] = '┘';
        }
        
        // Place cell content
        // (Implementation would iterate through table.cells and place text)
    }
}

/// Enhanced markdown extraction with spatial awareness
pub async fn get_markdown_content(pdf_path: &Path, page_num: usize) -> Result<String> {
    // Extract character-level data
    let extraction_data = tokio::task::spawn_blocking({
        let pdf_path = pdf_path.to_path_buf();
        move || extract_characters_with_coordinates(&pdf_path, page_num)
    }).await??;
    
    // Build markdown from spatially-aware extraction
    let mut markdown = String::new();
    
    // Create spatial mapper for analysis
    let mut mapper = SpatialGridMapper::new(
        100, // Dummy width for analysis
        100, // Dummy height for analysis
        extraction_data,
    );
    
    mapper.analyze_layout();
    
    // Calculate average font size for formatting decisions
    let avg_font_size = analysis::calculate_avg_font_size(&mapper.page_data.characters);
    
    // Process lines with proper formatting
    for line in &mapper.line_clusters {
        if line.is_empty() || line[0].is_empty() {
            continue;
        }
        
        let first_char = &line[0][0];
        
        // Detect formatting
        let is_header = analysis::is_header_char(first_char, avg_font_size);
        let is_italic = analysis::is_italic(first_char);
        let is_monospace = analysis::is_monospace(first_char);
        
        // Build line text
        let mut line_text = String::new();
        
        for (word_idx, word) in line.iter().enumerate() {
            if word_idx > 0 {
                line_text.push(' ');
            }
            
            for char in word {
                line_text.push(char.unicode);
            }
        }
        
        // Apply markdown formatting
        if is_header && first_char.font_size > avg_font_size * 1.5 {
            markdown.push_str(&format!("# {}\n\n", line_text.trim()));
        } else if is_header {
            markdown.push_str(&format!("## {}\n\n", line_text.trim()));
        } else if is_monospace {
            // Code block
            markdown.push_str("```\n");
            markdown.push_str(&line_text);
            markdown.push_str("\n```\n\n");
        } else if is_italic {
            markdown.push_str(&format!("*{}*\n\n", line_text.trim()));
        } else {
            // Regular paragraph
            markdown.push_str(&format!("{}\n\n", line_text.trim()));
        }
    }
    
    // Add table regions as markdown tables
    for table in &mapper.table_regions {
        markdown.push_str("\n| ");
        for cell in &table.cells[0] {
            markdown.push_str(cell);
            markdown.push_str(" | ");
        }
        markdown.push_str("\n|");
        for _ in &table.cells[0] {
            markdown.push_str(" --- |");
        }
        markdown.push('\n');
        
        for row in table.cells.iter().skip(1) {
            markdown.push_str("| ");
            for cell in row {
                markdown.push_str(cell);
                markdown.push_str(" | ");
            }
            markdown.push('\n');
        }
        markdown.push('\n');
    }
    
    if markdown.is_empty() {
        markdown = "# 📄 No Content Found\n\n> No text content could be extracted from this page.\n\n**Note:** This page may contain only images or scanned content.".to_string();
    }
    
    Ok(markdown)
}
