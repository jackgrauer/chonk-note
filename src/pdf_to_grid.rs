// Cargo.toml dependencies:
// [dependencies]
// rstar = "0.12"
// euclid = "0.22"
// grid = "0.14"
// anyhow = "1.0"

use anyhow::Result;
use euclid::{Point2D, Rect, Size2D, Scale};
use grid::Grid;
use rstar::{RTree, RTreeObject, AABB};
use std::cmp::{max, min};

// Define coordinate spaces for type safety
pub struct PdfSpace;
pub struct GridSpace;

pub type PdfPoint = Point2D<f32, PdfSpace>;
pub type PdfRect = Rect<f32, PdfSpace>;
pub type PdfSize = Size2D<f32, PdfSpace>;

pub type GridPoint = Point2D<usize, GridSpace>;
pub type GridRect = Rect<usize, GridSpace>;

/// A text block with position information
#[derive(Clone, Debug)]
pub struct TextBlock {
    pub text: String,
    pub bounds: PdfRect,
    pub font_size: f32,
    pub priority: i32,  // Higher priority text overwrites lower
}

impl RTreeObject for TextBlock {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.bounds.origin.x, self.bounds.origin.y],
            [self.bounds.origin.x + self.bounds.size.width,
             self.bounds.origin.y + self.bounds.size.height]
        )
    }
}

/// Converts PDF coordinates to terminal grid coordinates
pub struct CoordinateMapper {
    pdf_size: PdfSize,
    grid_size: (usize, usize),
    scale: Scale<f32, PdfSpace, GridSpace>,
    margin: f32,  // PDF units margin
}

impl CoordinateMapper {
    pub fn new(pdf_width: f32, pdf_height: f32, term_cols: usize, term_rows: usize) -> Self {
        let scale_x = term_cols as f32 / pdf_width;
        let scale_y = term_rows as f32 / pdf_height;
        
        // Use uniform scaling to maintain aspect ratio
        let uniform_scale = scale_x.min(scale_y);
        
        Self {
            pdf_size: PdfSize::new(pdf_width, pdf_height),
            grid_size: (term_cols, term_rows),
            scale: Scale::new(uniform_scale),
            margin: 2.0,
        }
    }
    
    /// Convert PDF point to grid coordinates
    pub fn pdf_to_grid(&self, point: PdfPoint) -> GridPoint {
        let scaled = point * self.scale.get();
        GridPoint::new(
            (scaled.x as usize).min(self.grid_size.0.saturating_sub(1)),
            (scaled.y as usize).min(self.grid_size.1.saturating_sub(1))
        )
    }
    
    /// Convert PDF rectangle to grid rectangle
    pub fn pdf_rect_to_grid(&self, rect: PdfRect) -> GridRect {
        let top_left = self.pdf_to_grid(rect.origin);
        let bottom_right = self.pdf_to_grid(
            PdfPoint::new(
                rect.origin.x + rect.size.width,
                rect.origin.y + rect.size.height
            )
        );
        
        GridRect::new(
            top_left,
            Size2D::new(
                bottom_right.x.saturating_sub(top_left.x).max(1),
                bottom_right.y.saturating_sub(top_left.y).max(1)
            )
        )
    }
    
    /// Check if a PDF point is within margins
    pub fn is_within_margins(&self, point: PdfPoint) -> bool {
        point.x >= self.margin 
            && point.y >= self.margin
            && point.x <= self.pdf_size.width - self.margin
            && point.y <= self.pdf_size.height - self.margin
    }
}

/// Cell content in the grid
#[derive(Clone, Debug)]
pub enum CellContent {
    Empty,
    Char(char),
    BoxDrawing(char),  // For table borders
}

impl Default for CellContent {
    fn default() -> Self {
        CellContent::Empty
    }
}

/// Main spatial text grid
pub struct SpatialTextGrid {
    grid: Grid<CellContent>,
    mapper: CoordinateMapper,
    text_blocks: Vec<TextBlock>,
    rtree: RTree<TextBlock>,
    allow_overlap: bool,
}

impl SpatialTextGrid {
    pub fn new(pdf_width: f32, pdf_height: f32, term_cols: usize, term_rows: usize) -> Self {
        Self {
            grid: Grid::new(term_rows, term_cols),
            mapper: CoordinateMapper::new(pdf_width, pdf_height, term_cols, term_rows),
            text_blocks: Vec::new(),
            rtree: RTree::new(),
            allow_overlap: false,
        }
    }
    
    /// Enable/disable text overlap
    pub fn set_allow_overlap(&mut self, allow: bool) {
        self.allow_overlap = allow;
    }
    
    /// Add a text block to be placed
    pub fn add_text_block(&mut self, text: &str, x: f32, y: f32, width: f32, height: f32, font_size: f32) {
        let block = TextBlock {
            text: text.to_string(),
            bounds: PdfRect::new(PdfPoint::new(x, y), PdfSize::new(width, height)),
            font_size,
            priority: (font_size * 10.0) as i32,  // Larger text has higher priority
        };
        self.text_blocks.push(block);
    }
    
    /// Process all text blocks and place them in the grid
    pub fn layout(&mut self) -> Result<()> {
        // Sort by priority (larger text first)
        self.text_blocks.sort_by_key(|b| -b.priority);
        
        // Build R-tree for collision detection
        self.rtree = RTree::bulk_load(self.text_blocks.clone());
        
        // Clone the blocks to avoid borrow issues
        let blocks_to_place = self.text_blocks.clone();
        
        // Place each text block
        for block in &blocks_to_place {
            self.place_text_block(block)?;
        }
        
        Ok(())
    }
    
    /// Place a single text block in the grid
    fn place_text_block(&mut self, block: &TextBlock) -> Result<()> {
        let grid_rect = self.mapper.pdf_rect_to_grid(block.bounds);
        let start_col = grid_rect.origin.x;
        let start_row = grid_rect.origin.y;
        
        // Check for collisions if overlap not allowed
        if !self.allow_overlap {
            let envelope = AABB::from_corners(
                [block.bounds.origin.x, block.bounds.origin.y],
                [block.bounds.origin.x + block.bounds.size.width,
                 block.bounds.origin.y + block.bounds.size.height]
            );
            
            let collisions = self.rtree.locate_in_envelope(&envelope);
            for collision in collisions {
                if collision.priority >= block.priority {
                    // Skip this block if a higher priority block is already here
                    return Ok(());
                }
            }
        }
        
        // Place the text character by character
        let mut col = start_col;
        let mut row = start_row;
        
        for ch in block.text.chars() {
            if ch == '\n' {
                row += 1;
                col = start_col;
                continue;
            }
            
            if row < self.grid.rows() && col < self.grid.cols() {
                // Check if we should overwrite
                let should_place = match &self.grid[(row, col)] {
                    CellContent::Empty => true,
                    _ => self.allow_overlap,
                };
                
                if should_place {
                    self.grid[(row, col)] = CellContent::Char(ch);
                }
            }
            
            col += 1;
            
            // Wrap to next line if needed
            if col >= self.grid.cols() || col >= start_col + grid_rect.size.width {
                row += 1;
                col = start_col;
            }
        }
        
        Ok(())
    }
    
    /// Draw a table border
    pub fn draw_table(&mut self, x: f32, y: f32, width: f32, height: f32, cells: Vec<Vec<String>>) -> Result<()> {
        let top_left = self.mapper.pdf_to_grid(PdfPoint::new(x, y));
        let bottom_right = self.mapper.pdf_to_grid(PdfPoint::new(x + width, y + height));
        
        let grid_width = bottom_right.x - top_left.x;
        let grid_height = bottom_right.y - top_left.y;
        
        // Draw corners
        self.grid[(top_left.y, top_left.x)] = CellContent::BoxDrawing('┌');
        self.grid[(top_left.y, bottom_right.x)] = CellContent::BoxDrawing('┐');
        self.grid[(bottom_right.y, top_left.x)] = CellContent::BoxDrawing('└');
        self.grid[(bottom_right.y, bottom_right.x)] = CellContent::BoxDrawing('┘');
        
        // Draw horizontal lines
        for col in (top_left.x + 1)..bottom_right.x {
            self.grid[(top_left.y, col)] = CellContent::BoxDrawing('─');
            self.grid[(bottom_right.y, col)] = CellContent::BoxDrawing('─');
        }
        
        // Draw vertical lines
        for row in (top_left.y + 1)..bottom_right.y {
            self.grid[(row, top_left.x)] = CellContent::BoxDrawing('│');
            self.grid[(row, bottom_right.x)] = CellContent::BoxDrawing('│');
        }
        
        // Place cell content
        if !cells.is_empty() {
            let rows_per_cell = grid_height / cells.len();
            let cols_per_cell = if !cells[0].is_empty() {
                grid_width / cells[0].len()
            } else {
                grid_width
            };
            
            for (row_idx, row_cells) in cells.iter().enumerate() {
                for (col_idx, cell_text) in row_cells.iter().enumerate() {
                    let cell_y = top_left.y + 1 + (row_idx * rows_per_cell);
                    let cell_x = top_left.x + 1 + (col_idx * cols_per_cell);
                    
                    // Place cell text
                    for (i, ch) in cell_text.chars().take(cols_per_cell - 1).enumerate() {
                        if cell_y < self.grid.rows() && cell_x + i < self.grid.cols() {
                            self.grid[(cell_y, cell_x + i)] = CellContent::Char(ch);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Render the grid to a string
    pub fn render(&self) -> String {
        let mut output = String::with_capacity(self.grid.rows() * (self.grid.cols() + 1));
        
        for row in 0..self.grid.rows() {
            for col in 0..self.grid.cols() {
                let ch = match &self.grid[(row, col)] {
                    CellContent::Empty => ' ',
                    CellContent::Char(c) => *c,
                    CellContent::BoxDrawing(c) => *c,
                };
                output.push(ch);
            }
            output.push('\n');
        }
        
        output
    }
    
    /// Convert to simple character grid
    pub fn to_char_grid(&self) -> Vec<Vec<char>> {
        let mut char_grid = vec![vec![' '; self.grid.cols()]; self.grid.rows()];
        
        for row in 0..self.grid.rows() {
            for col in 0..self.grid.cols() {
                char_grid[row][col] = match &self.grid[(row, col)] {
                    CellContent::Empty => ' ',
                    CellContent::Char(c) => *c,
                    CellContent::BoxDrawing(c) => *c,
                };
            }
        }
        
        char_grid
    }
    
    /// Get text at a specific grid position
    pub fn get_text_at(&self, col: usize, row: usize) -> Option<String> {
        // Convert grid coordinates back to PDF
        let pdf_x = (col as f32) / self.mapper.scale.get();
        let pdf_y = (row as f32) / self.mapper.scale.get();
        
        let search_envelope = AABB::from_corners(
            [pdf_x - 1.0, pdf_y - 1.0],
            [pdf_x + 1.0, pdf_y + 1.0]
        );
        
        let nearby = self.rtree.locate_in_envelope(&search_envelope);
        nearby
            .min_by_key(|block| {
                // Find closest block
                let center = block.bounds.center();
                ((center.x - pdf_x).powi(2) + (center.y - pdf_y).powi(2)) as i32
            })
            .map(|block| block.text.clone())
    }
    
    /// Clear the grid
    pub fn clear(&mut self) {
        self.grid.clear();
        self.text_blocks.clear();
        self.rtree = RTree::new();
    }
}