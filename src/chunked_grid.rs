// CHUNKED GRID - Minecraft-style infinite canvas
// Each chunk is 32x32 characters, loaded/unloaded on demand
// This allows clicking/typing ANYWHERE in a truly infinite space

use std::collections::BTreeMap;

const CHUNK_SIZE: usize = 32;

/// A single chunk - 32x32 block of characters
#[derive(Debug, Clone)]
struct Chunk {
    // Sparse storage within the chunk
    cells: BTreeMap<(usize, usize), char>,
}

impl Chunk {
    fn new() -> Self {
        Self {
            cells: BTreeMap::new(),
        }
    }

    fn set(&mut self, local_row: usize, local_col: usize, ch: char) {
        if ch == ' ' || ch == '\n' || ch == '\r' {
            self.cells.remove(&(local_row, local_col));
        } else {
            self.cells.insert((local_row, local_col), ch);
        }
    }

    fn get(&self, local_row: usize, local_col: usize) -> char {
        self.cells.get(&(local_row, local_col)).copied().unwrap_or(' ')
    }

    fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Get all non-empty cells in this chunk
    fn cells(&self) -> impl Iterator<Item = ((usize, usize), char)> + '_ {
        self.cells.iter().map(|((r, c), ch)| ((*r, *c), *ch))
    }
}

/// Block selection - rectangular region
#[derive(Debug, Clone)]
pub struct BlockSelection {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

impl BlockSelection {
    pub fn new(start_row: usize, start_col: usize) -> Self {
        Self {
            start_row,
            start_col,
            end_row: start_row,
            end_col: start_col,
        }
    }

    pub fn update(&mut self, row: usize, col: usize) {
        self.end_row = row;
        self.end_col = col;
    }

    pub fn bounds(&self) -> (usize, usize, usize, usize) {
        let min_row = self.start_row.min(self.end_row);
        let max_row = self.start_row.max(self.end_row);
        let min_col = self.start_col.min(self.end_col);
        let max_col = self.start_col.max(self.end_col);
        (min_row, min_col, max_row, max_col)
    }

    pub fn contains(&self, row: usize, col: usize) -> bool {
        let (min_row, min_col, max_row, max_col) = self.bounds();
        row >= min_row && row <= max_row && col >= min_col && col <= max_col
    }
}

/// Infinite canvas using chunked storage
#[derive(Debug, Clone)]
pub struct ChunkedGrid {
    // Map of chunk coordinates to chunks
    // Chunk (0,0) covers positions (0,0) to (31,31)
    // Chunk (1,0) covers positions (32,0) to (63,31)
    chunks: BTreeMap<(i32, i32), Chunk>,

    // Track bounds for efficient operations
    min_chunk: (i32, i32),
    max_chunk: (i32, i32),

    // Block selection (Excel-style rectangular selection)
    pub selection: Option<BlockSelection>,
}

impl ChunkedGrid {
    pub fn new() -> Self {
        Self {
            chunks: BTreeMap::new(),
            min_chunk: (0, 0),
            max_chunk: (0, 0),
            selection: None,
        }
    }

    /// Convert world position to chunk coordinates and local position
    fn pos_to_chunk(row: usize, col: usize) -> ((i32, i32), (usize, usize)) {
        let chunk_row = (row / CHUNK_SIZE) as i32;
        let chunk_col = (col / CHUNK_SIZE) as i32;
        let local_row = row % CHUNK_SIZE;
        let local_col = col % CHUNK_SIZE;
        ((chunk_row, chunk_col), (local_row, local_col))
    }

    /// Convert chunk coordinates and local position back to world position
    fn chunk_to_pos(chunk: (i32, i32), local: (usize, usize)) -> (usize, usize) {
        let row = (chunk.0 as usize * CHUNK_SIZE) + local.0;
        let col = (chunk.1 as usize * CHUNK_SIZE) + local.1;
        (row, col)
    }

    /// Set character at ANY position - auto-creates chunks
    pub fn set(&mut self, row: usize, col: usize, ch: char) {
        let (chunk_pos, local_pos) = Self::pos_to_chunk(row, col);

        // Get or create chunk
        let chunk = self.chunks.entry(chunk_pos).or_insert_with(Chunk::new);
        chunk.set(local_pos.0, local_pos.1, ch);

        // If chunk is now empty, remove it
        if chunk.is_empty() {
            self.chunks.remove(&chunk_pos);
        } else {
            // Update bounds
            self.min_chunk.0 = self.min_chunk.0.min(chunk_pos.0);
            self.min_chunk.1 = self.min_chunk.1.min(chunk_pos.1);
            self.max_chunk.0 = self.max_chunk.0.max(chunk_pos.0);
            self.max_chunk.1 = self.max_chunk.1.max(chunk_pos.1);
        }
    }

    /// Get character at any position
    pub fn get(&self, row: usize, col: usize) -> char {
        let (chunk_pos, local_pos) = Self::pos_to_chunk(row, col);

        self.chunks
            .get(&chunk_pos)
            .map(|chunk| chunk.get(local_pos.0, local_pos.1))
            .unwrap_or(' ')
    }

    /// Insert string at position
    pub fn insert_at(&mut self, row: usize, col: usize, text: &str) {
        for (i, ch) in text.chars().enumerate() {
            if ch == '\n' {
                continue; // Skip newlines for now
            }
            self.set(row, col + i, ch);
        }
    }

    /// Delete character at position
    pub fn delete_at(&mut self, row: usize, col: usize) {
        self.set(row, col, ' ');
    }

    /// Get all chunks that intersect with a viewport
    pub fn get_visible_chunks(&self, viewport: Viewport) -> Vec<(i32, i32)> {
        let (start_chunk, _) = Self::pos_to_chunk(viewport.start_row, viewport.start_col);
        let (end_chunk, _) = Self::pos_to_chunk(viewport.end_row, viewport.end_col);

        let mut visible = Vec::new();
        for chunk_row in start_chunk.0..=end_chunk.0 {
            for chunk_col in start_chunk.1..=end_chunk.1 {
                let chunk_pos = (chunk_row, chunk_col);
                if self.chunks.contains_key(&chunk_pos) {
                    visible.push(chunk_pos);
                }
            }
        }
        visible
    }

    /// Get content for a specific line (for rendering)
    pub fn get_line(&self, row: usize, start_col: usize, end_col: usize) -> String {
        let mut line = String::new();
        for col in start_col..=end_col {
            line.push(self.get(row, col));
        }
        // Trim trailing spaces
        line.trim_end().to_string()
    }

    /// Get bounds of actual content
    pub fn bounds(&self) -> Option<(usize, usize, usize, usize)> {
        if self.chunks.is_empty() {
            return None;
        }

        let mut min_row = usize::MAX;
        let mut max_row = 0;
        let mut min_col = usize::MAX;
        let mut max_col = 0;

        for (&chunk_pos, chunk) in &self.chunks {
            for ((local_row, local_col), _) in chunk.cells() {
                let (row, col) = Self::chunk_to_pos(chunk_pos, (local_row, local_col));
                min_row = min_row.min(row);
                max_row = max_row.max(row);
                min_col = min_col.min(col);
                max_col = max_col.max(col);
            }
        }

        Some((min_row, min_col, max_row, max_col))
    }

    /// Count total non-empty cells
    pub fn cell_count(&self) -> usize {
        self.chunks.values().map(|chunk| chunk.cells.len()).sum()
    }

    /// Count loaded chunks
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Export to lines for saving (only exports content area)
    pub fn to_lines(&self) -> Vec<String> {
        let Some((min_row, min_col, max_row, max_col)) = self.bounds() else {
            return vec![String::new()];
        };

        let mut lines = Vec::new();
        for row in min_row..=max_row {
            let line = self.get_line(row, min_col, max_col);
            lines.push(line);
        }

        lines
    }

    /// Import from lines
    pub fn from_lines(lines: &[String]) -> Self {
        let mut grid = Self::new();

        for (row, line) in lines.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                if ch != ' ' {
                    grid.set(row, col, ch);
                }
            }
        }

        grid
    }

    /// Clear the entire grid
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.min_chunk = (0, 0);
        self.max_chunk = (0, 0);
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        self.to_lines().join("\n")
    }

    /// Create from string
    pub fn from_string(content: &str) -> Self {
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        Self::from_lines(&lines)
    }

    /// Start block selection at current position
    pub fn start_selection(&mut self, row: usize, col: usize) {
        self.selection = Some(BlockSelection::new(row, col));
    }

    /// Update selection to current position
    pub fn update_selection(&mut self, row: usize, col: usize) {
        if let Some(ref mut sel) = self.selection {
            sel.update(row, col);
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Copy selected block to string (Excel-style: preserves rectangular shape)
    pub fn copy_block(&self) -> Option<Vec<String>> {
        let sel = self.selection.as_ref()?;
        let (min_row, min_col, max_row, max_col) = sel.bounds();

        let mut lines = Vec::new();
        for row in min_row..=max_row {
            let mut line = String::new();
            for col in min_col..=max_col {
                line.push(self.get(row, col));
            }
            lines.push(line);
        }

        Some(lines)
    }

    /// Cut selected block (copy and clear the region)
    pub fn cut_block(&mut self) -> Option<Vec<String>> {
        let copied = self.copy_block();

        if let Some(ref sel) = self.selection {
            let (min_row, min_col, max_row, max_col) = sel.bounds();

            // Clear all cells in the rectangular region
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    self.set(row, col, ' ');
                }
            }
        }

        self.selection = None;
        copied
    }

    /// Paste block at position (Excel-style: rectangular paste without shifting)
    pub fn paste_block(&mut self, lines: &[String], row: usize, col: usize) {
        for (line_offset, line) in lines.iter().enumerate() {
            for (col_offset, ch) in line.chars().enumerate() {
                self.set(row + line_offset, col + col_offset, ch);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_positioning() {
        let mut grid = ChunkedGrid::new();

        // Set at various positions
        grid.set(0, 0, 'A');        // Chunk (0,0)
        grid.set(31, 31, 'B');      // Still chunk (0,0)
        grid.set(32, 0, 'C');       // Chunk (1,0)
        grid.set(0, 32, 'D');       // Chunk (0,1)
        grid.set(100, 100, 'E');    // Chunk (3,3)

        assert_eq!(grid.get(0, 0), 'A');
        assert_eq!(grid.get(31, 31), 'B');
        assert_eq!(grid.get(32, 0), 'C');
        assert_eq!(grid.get(0, 32), 'D');
        assert_eq!(grid.get(100, 100), 'E');

        // Check chunks created
        assert_eq!(grid.chunk_count(), 4); // 4 different chunks
    }

    #[test]
    fn test_insert_anywhere() {
        let mut grid = ChunkedGrid::new();

        // Insert at crazy position
        grid.insert_at(1000, 5000, "Hello");

        assert_eq!(grid.get(1000, 5000), 'H');
        assert_eq!(grid.get(1000, 5004), 'o');

        // Should have created chunk (31, 156)
        assert!(grid.chunk_count() > 0);
    }

    #[test]
    fn test_sparse_chunks() {
        let mut grid = ChunkedGrid::new();

        grid.set(0, 0, 'A');
        grid.set(1000, 1000, 'B');

        // Only 2 chunks should exist, not 32x32 chunks
        assert_eq!(grid.chunk_count(), 2);
        assert_eq!(grid.cell_count(), 2);
    }

    #[test]
    fn test_viewport_culling() {
        let mut grid = ChunkedGrid::new();

        // Spread content across many chunks
        for i in 0..10 {
            grid.set(i * 50, i * 50, 'X');
        }

        // Only get chunks in small viewport
        let viewport = Viewport {
            start_row: 0,
            start_col: 0,
            end_row: 100,
            end_col: 100,
        };

        let visible = grid.get_visible_chunks(viewport);

        // Should be much less than 10 chunks
        assert!(visible.len() < grid.chunk_count());
    }

    #[test]
    fn test_export_import() {
        let mut grid = ChunkedGrid::new();

        grid.insert_at(5, 10, "Hello");
        grid.insert_at(10, 20, "World");

        let lines = grid.to_lines();
        let grid2 = ChunkedGrid::from_lines(&lines);

        // Content should be preserved
        assert_eq!(grid2.get(5, 10), 'H');
        assert_eq!(grid2.get(10, 20), 'W');
    }
}
