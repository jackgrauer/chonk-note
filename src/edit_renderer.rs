// VIEWPORT AND SCROLLING - WHY IT'S SO FRAGILE
// =============================================
//
// The viewport system has multiple coordinate systems that don't play well:
//
// 1. BUFFER SIZE: The actual extracted text (e.g., 400x200)
// 2. VIEWPORT SIZE: The terminal panel size (e.g., term_width/2 x term_height)
// 3. DISPLAY COORDINATES: Where on screen to render (with borders, headers)
// 4. SCROLL OFFSETS: How much we've scrolled in X and Y
//
// FRAGILITY ISSUES:
// - Buffer gets resized on every update_buffer() call
// - Viewport size changes when terminal resizes but buffer doesn't know
// - Scroll bounds calculation is inconsistent
// - Mouse events use screen coords, need translation to buffer coords
// - No validation that scroll position is still valid after resize
//
// COMMON BREAKS:
// - Scroll past end of content -> panic or garbage display
// - Terminal resize -> scroll position now out of bounds
// - Buffer smaller than viewport -> negative scroll bounds
// - Mouse click outside buffer bounds -> index out of range

use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, SetForegroundColor, SetBackgroundColor, ResetColor},
};
use std::io::{self, Write};

pub struct EditPanelRenderer {
    buffer: Vec<Vec<char>>,      // The full extracted content
    viewport_width: u16,          // Display panel width (terminal constrained)
    viewport_height: u16,         // Display panel height (terminal constrained)
    scroll_x: u16,               // Horizontal scroll offset
    scroll_y: u16,               // Vertical scroll offset
}

impl EditPanelRenderer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            buffer: vec![vec![' '; width as usize]; height as usize],
            viewport_width: width,
            viewport_height: height,
            scroll_x: 0,
            scroll_y: 0,
        }
    }
    
    pub fn update_buffer(&mut self, matrix: &[Vec<char>]) {
        self.buffer.clear();
        for row in matrix {
            self.buffer.push(row.clone());
        }
    }
    
    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_y = self.scroll_y.saturating_sub(lines);
    }
    
    pub fn scroll_down(&mut self, lines: u16) {
        let max_scroll = self.buffer.len().saturating_sub(self.viewport_height as usize) as u16;
        self.scroll_y = (self.scroll_y + lines).min(max_scroll);
    }
    
    pub fn scroll_left(&mut self, cols: u16) {
        self.scroll_x = self.scroll_x.saturating_sub(cols);
    }
    
    pub fn scroll_right(&mut self, cols: u16) {
        let max_width = self.buffer.get(0).map(|r| r.len()).unwrap_or(0);
        let max_scroll = max_width.saturating_sub(self.viewport_width as usize) as u16;
        self.scroll_x = (self.scroll_x + cols).min(max_scroll);
    }
    
    pub fn scroll_to_x(&mut self, x: u16) {
        self.scroll_x = x;
    }
    
    pub fn scroll_to_y(&mut self, y: u16) {
        self.scroll_y = y;
    }
    
    /// Efficiently render the text buffer to the terminal within bounds
    pub fn render(&self, start_x: u16, start_y: u16, max_width: u16, max_height: u16) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        // Clamp rendering to the specified bounds
        let render_width = self.viewport_width.min(max_width);
        let render_height = self.viewport_height.min(max_height);
        
        // Build the entire screen content in one go
        let mut screen_buffer = String::with_capacity(
            (render_width * render_height * 2) as usize
        );
        
        for y in 0..render_height {
            let buffer_y = (self.scroll_y + y) as usize;
            
            // Move cursor to start of line
            execute!(stdout, MoveTo(start_x, start_y + y))?;
            
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
            execute!(stdout, MoveTo(start_x, start_y + y))?;
            
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
                        execute!(
                            stdout,
                            SetBackgroundColor(Color::Yellow),
                            SetForegroundColor(Color::Black),
                            Print(row[x]),
                            ResetColor
                        )?;
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
        use crate::theme::ChonkerTheme;
        
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
            execute!(stdout, MoveTo(start_x, start_y + y))?;
            
            if buffer_y < self.buffer.len() {
                let row = &self.buffer[buffer_y];
                let start_col = self.scroll_x as usize;
                let end_col = (start_col + render_width as usize).min(row.len());
                
                for x in start_col..end_col {
                    let is_cursor = cursor == (x, buffer_y);
                    
                    // Check if position is in selection
                    let is_selected = if let Some(((sel_start_row, sel_start_col), (sel_end_row, sel_end_col))) = selection_bounds {
                        (buffer_y > sel_start_row || (buffer_y == sel_start_row && x >= sel_start_col)) &&
                        (buffer_y < sel_end_row || (buffer_y == sel_end_row && x <= sel_end_col))
                    } else {
                        false
                    };
                    
                    let ch = row.get(x).copied().unwrap_or(' ');
                    
                    if is_cursor {
                        // Cursor - bright background
                        execute!(
                            stdout,
                            SetBackgroundColor(ChonkerTheme::accent_text()),
                            SetForegroundColor(Color::Black),
                            Print(ch),
                            ResetColor
                        )?;
                    } else if is_selected {
                        // Selection - dimmer background
                        execute!(
                            stdout,
                            SetBackgroundColor(Color::DarkBlue),
                            SetForegroundColor(Color::White),
                            Print(ch),
                            ResetColor
                        )?;
                    } else {
                        // Normal character
                        write!(stdout, "{}", ch)?;
                    }
                }
                
                // Clear rest of line if needed
                let chars_written = end_col - start_col;
                if chars_written < render_width as usize {
                    // Check if cursor is at end of line
                    let is_cursor_at_eol = cursor == (row.len(), buffer_y) && 
                                         row.len() >= start_col && row.len() < end_col;
                    
                    if is_cursor_at_eol {
                        // Show cursor at end of line
                        execute!(
                            stdout,
                            SetBackgroundColor(ChonkerTheme::accent_text()),
                            Print(" "),
                            ResetColor
                        )?;
                        
                        // Fill remaining space
                        if chars_written + 1 < render_width as usize {
                            write!(stdout, "{:width$}", "", width = (render_width as usize - chars_written - 1))?;
                        }
                    } else {
                        // Fill with spaces
                        write!(stdout, "{:width$}", "", width = (render_width as usize - chars_written))?;
                    }
                }
            } else {
                // Empty line - check if cursor is here
                let is_cursor_at_empty_line = cursor == (0, buffer_y);
                
                if is_cursor_at_empty_line {
                    // Show cursor on empty line
                    execute!(
                        stdout,
                        SetBackgroundColor(ChonkerTheme::accent_text()),
                        Print(" "),
                        ResetColor
                    )?;
                    
                    // Fill remaining space
                    if render_width > 1 {
                        write!(stdout, "{:width$}", "", width = (render_width as usize - 1))?;
                    }
                } else {
                    // Fill entire line with spaces
                    write!(stdout, "{:width$}", "", width = render_width as usize)?;
                }
            }
        }
        
        stdout.flush()?;
        Ok(())
    }
}