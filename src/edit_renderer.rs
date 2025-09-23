// HELIX-CORE POWERED VIEWPORT - ROBUST AND STABLE
// ===============================================
// Now powered by helix-core Rope system - no more fragility issues!

// CROSSTERM ELIMINATED! Pure ANSI escape sequences
use std::io::{self, Write};
use helix_core::Rope;
use crate::block_selection::BlockSelection;

pub struct EditPanelRenderer {
    buffer: Vec<Vec<char>>,      // The full extracted content
    viewport_width: u16,          // Display panel width (terminal constrained)
    viewport_height: u16,         // Display panel height (terminal constrained)
    pub scroll_x: u16,               // Horizontal scroll offset
    pub scroll_y: u16,               // Vertical scroll offset
    pub viewport_x: usize,           // Current viewport X position for mouse mapping
    pub viewport_y: usize,           // Current viewport Y position for mouse mapping
}

impl EditPanelRenderer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            buffer: vec![vec![' '; width as usize]; height as usize],
            viewport_width: width,
            viewport_height: height,
            scroll_x: 0,
            scroll_y: 0,
            viewport_x: 0,
            viewport_y: 0,
        }
    }
    
    // update_buffer eliminated - using update_from_rope with helix-core

    // HELIX-CORE INTEGRATION! Convert Rope to display format
    pub fn update_from_rope(&mut self, rope: &Rope) {
        self.buffer.clear();

        // Convert Rope back to rendering format
        for line in rope.lines() {
            let mut row: Vec<char> = line.chars()
                .filter(|&ch| ch != '\n' && ch != '\r')
                .collect();

            // Pad to width if needed
            while row.len() < self.viewport_width as usize {
                row.push(' ');
            }
            self.buffer.push(row);
        }
    }
    
    /// Update viewport dimensions (for zoom functionality)
    pub fn set_viewport_size(&mut self, width: u16, height: u16) {
        self.viewport_width = width;
        self.viewport_height = height;
    }


    pub fn scroll_up(&mut self, lines: u16) {
        // Hard boundary at top - never go negative
        self.scroll_y = self.scroll_y.saturating_sub(lines);
        self.viewport_y = self.scroll_y as usize;
    }

    pub fn scroll_down(&mut self, lines: u16) {
        let max_scroll = self.buffer.len().saturating_sub(self.viewport_height as usize) as u16;
        self.scroll_y = (self.scroll_y + lines).min(max_scroll);
        self.viewport_y = self.scroll_y as usize;
    }

    pub fn scroll_left(&mut self, cols: u16) {
        // Hard boundary at left - never go negative
        self.scroll_x = self.scroll_x.saturating_sub(cols);
        self.viewport_x = self.scroll_x as usize;
    }

    pub fn scroll_right(&mut self, cols: u16) {
        let max_width = self.buffer.get(0).map(|r| r.len()).unwrap_or(0);
        let max_scroll = max_width.saturating_sub(self.viewport_width as usize) as u16;
        self.scroll_x = (self.scroll_x + cols).min(max_scroll);
        self.viewport_x = self.scroll_x as usize;
    }

    pub fn scroll_to_x(&mut self, x: u16) {
        // Enforce boundaries when setting scroll position directly
        let max_width = self.buffer.get(0).map(|r| r.len()).unwrap_or(0);
        let max_scroll = max_width.saturating_sub(self.viewport_width as usize) as u16;
        self.scroll_x = x.min(max_scroll);
        self.viewport_x = self.scroll_x as usize;
    }

    pub fn scroll_to_y(&mut self, y: u16) {
        // Enforce boundaries when setting scroll position directly
        let max_scroll = self.buffer.len().saturating_sub(self.viewport_height as usize) as u16;
        self.scroll_y = y.min(max_scroll);
        self.viewport_y = self.scroll_y as usize;
    }

    /// Automatically scroll viewport to follow cursor with padding
    /// IMPORTANT: Viewport must ALWAYS keep cursor visible within its boundaries
    pub fn follow_cursor(&mut self, cursor_x: usize, cursor_y: usize, padding: u16) {
        let cursor_x = cursor_x as u16;
        let cursor_y = cursor_y as u16;

        // HARD BOUNDARIES: Never allow negative scroll positions
        const MIN_SCROLL: u16 = 0;

        // VERTICAL SCROLLING - Ensure cursor is always visible vertically

        // If cursor is above viewport (including padding), scroll up to show it
        if cursor_y < self.scroll_y + padding {
            // Never scroll past 0 (hard boundary at top)
            self.scroll_y = cursor_y.saturating_sub(padding).max(MIN_SCROLL);
        }
        // If cursor is below viewport (including padding), scroll down to show it
        else if cursor_y >= self.scroll_y + self.viewport_height.saturating_sub(padding) {
            // Calculate minimum scroll needed to show cursor with padding
            let min_scroll = cursor_y.saturating_sub(self.viewport_height.saturating_sub(padding + 1));
            let max_scroll = self.buffer.len().saturating_sub(self.viewport_height as usize) as u16;
            self.scroll_y = min_scroll.min(max_scroll);
        }

        // HORIZONTAL SCROLLING - Ensure cursor is always visible horizontally

        // If cursor is left of viewport (including padding), scroll left to show it
        if cursor_x < self.scroll_x + padding {
            // Never scroll past 0 (hard boundary at left)
            self.scroll_x = cursor_x.saturating_sub(padding).max(MIN_SCROLL);
        }
        // If cursor is right of viewport (including padding), scroll right to show it
        else if cursor_x >= self.scroll_x + self.viewport_width.saturating_sub(padding) {
            // Calculate minimum scroll needed to show cursor with padding
            let min_scroll = cursor_x.saturating_sub(self.viewport_width.saturating_sub(padding + 1));
            let max_width = self.buffer.get(0).map(|r| r.len()).unwrap_or(0);
            let max_scroll = max_width.saturating_sub(self.viewport_width as usize) as u16;
            self.scroll_x = min_scroll.min(max_scroll);
        }

        // FINAL SAFETY CHECK: Ensure viewport boundaries are valid
        // The viewport can never be positioned where cursor would be outside its range

        // If cursor is somehow still not visible, force viewport to contain it
        if cursor_y < self.scroll_y {
            self.scroll_y = cursor_y;  // Force viewport to contain cursor
        } else if cursor_y >= self.scroll_y + self.viewport_height {
            self.scroll_y = cursor_y.saturating_sub(self.viewport_height - 1);
        }

        if cursor_x < self.scroll_x {
            self.scroll_x = cursor_x;  // Force viewport to contain cursor
        } else if cursor_x >= self.scroll_x + self.viewport_width {
            self.scroll_x = cursor_x.saturating_sub(self.viewport_width - 1);
        }

        // Ensure scroll positions are never negative (absolute hard boundary)
        self.scroll_x = self.scroll_x.max(MIN_SCROLL);
        self.scroll_y = self.scroll_y.max(MIN_SCROLL);

        // Update viewport position for mouse mapping
        self.viewport_x = self.scroll_x as usize;
        self.viewport_y = self.scroll_y as usize;
    }
    
    /// Efficiently render the text buffer to the terminal within bounds
    pub fn render(&self, start_x: u16, start_y: u16, max_width: u16, max_height: u16) -> io::Result<()> {
        self.render_with_label(start_x, start_y, max_width, max_height, None)
    }

    /// Render with an optional extraction method label
    pub fn render_with_label(&self, start_x: u16, start_y: u16, max_width: u16, max_height: u16, method_label: Option<&str>) -> io::Result<()> {
        let mut stdout = io::stdout();

        // Clamp rendering to the specified bounds
        let render_width = self.viewport_width.min(max_width);
        let render_height = self.viewport_height.min(max_height);

        // Build the entire screen content in one go
        let mut screen_buffer = String::with_capacity(
            (render_width * render_height * 2) as usize
        );

        // Add extraction method label at the top if provided
        let mut start_row = 0;
        if let Some(label) = method_label {
            // Move cursor to top of edit panel
            print!("\x1b[{};{}H", start_y + 1, start_x + 1);

            // Create label with styling
            let label_text = format!(" [{}] ", label);
            let padding = render_width.saturating_sub(label_text.len() as u16);

            // Render label with subtle background color
            print!("\x1b[48;2;40;40;40m\x1b[38;2;200;200;200m{}{}\x1b[0m",
                   label_text,
                   " ".repeat(padding as usize));

            start_row = 1; // Start actual content from row 1
        }

        for y in start_row..render_height {
            let buffer_y = (self.scroll_y + y - start_row) as usize;
            
            // Move cursor to start of line
            // ANSI: Move cursor to position
            print!("\x1b[{};{}H", start_y + y + 1, start_x + 1);  // 1-based coordinates
            
            if buffer_y < self.buffer.len() {
                let row = &self.buffer[buffer_y];
                let start_col = self.scroll_x as usize;
                let end_col = (start_col + render_width as usize).min(row.len());
                
                // Build the entire line at once, but truncate to render_width
                screen_buffer.clear();
                for x in start_col..end_col {
                    screen_buffer.push(row[x]);
                }
                
                // Pad with spaces if needed
                let chars_written = end_col - start_col;
                if chars_written < render_width as usize {
                    for _ in chars_written..render_width as usize {
                        screen_buffer.push(' ');
                    }
                }
                
                // Write the entire line in one go
                write!(stdout, "{}", screen_buffer)?;
            } else {
                // Clear the rest of the viewport
                write!(stdout, "{:width$}", "", width = render_width as usize)?;
            }
        }
        
        stdout.flush()?;
        Ok(())
    }
    
    /// Render with highlighting for search results or selections
    pub fn render_with_highlights(
        &self,
        start_x: u16,
        start_y: u16,
        highlights: &[(usize, usize, usize, usize)], // (start_y, start_x, end_y, end_x)
    ) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        for y in 0..self.viewport_height {
            let buffer_y = (self.scroll_y + y) as usize;
            // ANSI: Move cursor to position
            print!("\x1b[{};{}H", start_y + y + 1, start_x + 1);  // 1-based coordinates
            
            if buffer_y < self.buffer.len() {
                let row = &self.buffer[buffer_y];
                let start_col = self.scroll_x as usize;
                let end_col = (start_col + self.viewport_width as usize).min(row.len());
                
                for x in start_col..end_col {
                    let is_highlighted = highlights.iter().any(|(sy, sx, ey, ex)| {
                        (buffer_y > *sy || (buffer_y == *sy && x >= *sx)) &&
                        (buffer_y < *ey || (buffer_y == *ey && x <= *ex))
                    });
                    
                    if is_highlighted {
                        // ANSI: Selection highlighting
                        print!("\x1b[48;2;0;0;139m\x1b[38;2;255;255;255m{}\x1b[m", row[x]);
                    } else {
                        write!(stdout, "{}", row[x])?;
                    }
                }
                
                // Clear rest of line
                let chars_written = end_col - start_col;
                if chars_written < self.viewport_width as usize {
                    write!(stdout, "{:width$}", "", width = (self.viewport_width as usize - chars_written))?;
                }
            } else {
                write!(stdout, "{:width$}", "", width = self.viewport_width as usize)?;
            }
        }
        
        stdout.flush()?;
        Ok(())
    }
    
    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport_width = width;
        self.viewport_height = height;
    }
    
    /// Get current scroll position for cursor/selection calculations
    pub fn get_scroll(&self) -> (u16, u16) {
        (self.scroll_x, self.scroll_y)
    }
    
    pub fn get_viewport_size(&self) -> (u16, u16) {
        (self.viewport_width, self.viewport_height)
    }

    /// Draw scrollbars for the text editor viewport
    pub fn draw_scrollbars(&self, start_x: u16, start_y: u16, width: u16, height: u16) -> io::Result<()> {
        // Calculate content dimensions
        let content_height = self.buffer.len() as u16;
        let content_width = self.buffer.iter().map(|row| row.len()).max().unwrap_or(0) as u16;

        // Draw horizontal scrollbar if content is wider than viewport
        if content_width > width {
            let scrollbar_y = start_y + height;
            let thumb_width = ((width as f32 / content_width as f32) * width as f32).max(2.0) as u16;
            let max_scroll = content_width.saturating_sub(width);
            let thumb_pos = if max_scroll > 0 {
                ((self.scroll_x as f32 / max_scroll as f32) * (width - thumb_width) as f32) as u16
            } else {
                0
            };

            // Draw scrollbar track
            print!("\x1b[{};{}H\x1b[38;2;40;40;40m{}\x1b[0m",
                scrollbar_y, start_x + 1, "─".repeat(width as usize));
            // Draw scrollbar thumb
            print!("\x1b[{};{}H\x1b[38;2;100;100;100m{}\x1b[0m",
                scrollbar_y, start_x + thumb_pos + 1, "═".repeat(thumb_width as usize));
        }

        // Draw vertical scrollbar if content is taller than viewport
        if content_height > height {
            let scrollbar_x = start_x + width;
            let thumb_height = ((height as f32 / content_height as f32) * height as f32).max(2.0) as u16;
            let max_scroll = content_height.saturating_sub(height);
            let thumb_pos = if max_scroll > 0 {
                ((self.scroll_y as f32 / max_scroll as f32) * (height - thumb_height) as f32) as u16
            } else {
                0
            };

            // Draw scrollbar track and thumb
            for y in 0..height {
                if y >= thumb_pos && y < thumb_pos + thumb_height {
                    print!("\x1b[{};{}H\x1b[38;2;100;100;100m║\x1b[0m", start_y + y + 1, scrollbar_x);
                } else {
                    print!("\x1b[{};{}H\x1b[38;2;40;40;40m│\x1b[0m", start_y + y + 1, scrollbar_x);
                }
            }
        }

        Ok(())
    }
    
    /// Render with block selection (rectangular selection)
    pub fn render_with_block_selection(
        &self,
        start_x: u16,
        start_y: u16,
        max_width: u16,
        max_height: u16,
        cursor: (usize, usize),
        block_selection: Option<&BlockSelection>,
    ) -> io::Result<()> {
        let mut stdout = io::stdout();

        // Clamp rendering to the specified bounds
        let render_width = self.viewport_width.min(max_width);
        let render_height = self.viewport_height.min(max_height);

        // Process block selection bounds if present
        let block_bounds = if let Some(block_sel) = block_selection {
            let ((min_line, min_col), (max_line, max_col)) = block_sel.visual_bounds();
            Some((min_col, min_line, max_col, max_line))
        } else {
            None
        };

        for y in 0..render_height {
            let buffer_y = (self.scroll_y + y) as usize;

            // Move cursor to start of line
            print!("\x1b[{};{}H", start_y + y + 1, start_x + 1);  // 1-based coordinates

            if buffer_y < self.buffer.len() {
                let row = &self.buffer[buffer_y];
                let start_col = self.scroll_x as usize;
                let end_col = (start_col + render_width as usize).min(row.len());

                // Render characters that exist in the line
                for x in start_col..end_col {
                    let is_cursor = cursor.1 == buffer_y && cursor.0 == x;

                    // Check if position is in block selection
                    let is_in_block = if let Some((min_col, min_line, max_col, max_line)) = block_bounds {
                        buffer_y >= min_line && buffer_y <= max_line &&
                        x >= min_col && x <= max_col
                    } else {
                        false
                    };

                    let ch = row.get(x).copied().unwrap_or(' ');

                    if is_cursor {
                        // ANSI: Cursor highlighting (light color)
                        print!("\x1b[48;2;80;80;200m{}\x1b[m", ch);
                    } else if is_in_block {
                        // ANSI: Block selection highlighting
                        print!("\x1b[48;2;80;80;200m\x1b[38;2;255;255;255m{}\x1b[m", ch);
                    } else {
                        // Normal character
                        write!(stdout, "{}", ch)?;
                    }
                }

                // Handle the rest of the line (including virtual cursor position)
                let chars_written = end_col - start_col;
                if chars_written < render_width as usize {
                    let remaining_space = render_width as usize - chars_written;

                    // Check if cursor is in the virtual space (past line end)
                    for offset in 0..remaining_space {
                        let virtual_x = end_col + offset;
                        if cursor.1 == buffer_y && cursor.0 == virtual_x {
                            // Render cursor in virtual space
                            print!("\x1b[48;2;80;80;200m \x1b[m");
                        } else {
                            write!(stdout, " ")?;
                        }
                    }
                }
            } else {
                // Clear the rest of the viewport
                write!(stdout, "{:width$}", "", width = render_width as usize)?;
            }
        }


        stdout.flush()?;
        Ok(())
    }

    /// Render with cursor and selection highlighting
    pub fn render_with_cursor_and_selection(
        &self,
        start_x: u16,
        start_y: u16,
        max_width: u16,
        max_height: u16,
        cursor: (usize, usize),
        selection_start: Option<(usize, usize)>,
        selection_end: Option<(usize, usize)>,
    ) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        // Clamp rendering to the specified bounds
        let render_width = self.viewport_width.min(max_width);
        let render_height = self.viewport_height.min(max_height);
        
        // Calculate selection bounds if we have both start and end
        let selection_bounds = if let (Some(start), Some(end)) = (selection_start, selection_end) {
            let (start_row, start_col) = start;
            let (end_row, end_col) = end;
            
            // Normalize selection (ensure start comes before end)
            if start_row < end_row || (start_row == end_row && start_col < end_col) {
                Some(((start_row, start_col), (end_row, end_col)))
            } else {
                Some(((end_row, end_col), (start_row, start_col)))
            }
        } else {
            None
        };
        
        for y in 0..render_height {
            let buffer_y = (self.scroll_y + y) as usize;
            
            // Move cursor to start of line
            // ANSI: Move cursor to position
            print!("\x1b[{};{}H", start_y + y + 1, start_x + 1);  // 1-based coordinates
            
            if buffer_y < self.buffer.len() {
                let row = &self.buffer[buffer_y];
                let start_col = self.scroll_x as usize;
                let end_col = (start_col + render_width as usize).min(row.len());
                
                for x in start_col..end_col {
                    let is_cursor = cursor.1 == buffer_y && cursor.0 == x;
                    
                    // Check if position is in selection
                    let is_selected = if let Some(((sel_start_row, sel_start_col), (sel_end_row, sel_end_col))) = selection_bounds {
                        (buffer_y > sel_start_row || (buffer_y == sel_start_row && x >= sel_start_col)) &&
                        (buffer_y < sel_end_row || (buffer_y == sel_end_row && x <= sel_end_col))
                    } else {
                        false
                    };
                    
                    let ch = row.get(x).copied().unwrap_or(' ');
                    
                    if is_cursor {
                        // ANSI: Cursor highlighting (light color)
                        print!("\x1b[48;2;80;80;200m{}\x1b[m", ch);
                    } else if is_selected {
                        // ANSI: Selection highlighting (same blue as block selection)
                        print!("\x1b[48;2;80;80;200m\x1b[38;2;255;255;255m{}\x1b[m", ch);
                    } else {
                        // Normal character
                        write!(stdout, "{}", ch)?;
                    }
                }
                
                // Clear rest of line if needed and handle virtual cursor
                let chars_written = end_col - start_col;
                if chars_written < render_width as usize {
                    let remaining_space = render_width as usize - chars_written;
                    // Check if cursor is past line end (virtual position)
                    for offset in 0..remaining_space {
                        let virtual_x = end_col + offset;
                        if cursor.1 == buffer_y && cursor.0 == virtual_x {
                            // Render cursor in virtual space
                            print!("\x1b[48;2;80;80;200m \x1b[m");
                        } else {
                            write!(stdout, " ")?;
                        }
                    }
                }
            } else {
                // Empty line - check if cursor is here
                for x in 0..render_width as usize {
                    if cursor.1 == buffer_y && cursor.0 == x {
                        // Render cursor on empty line
                        print!("\x1b[48;2;80;80;200m \x1b[m");
                    } else {
                        write!(stdout, " ")?;
                    }
                }
            }
        }


        stdout.flush()?;
        Ok(())
    }
}